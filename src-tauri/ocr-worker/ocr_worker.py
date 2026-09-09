import argparse
import contextlib
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter, ImageSequence
import rapidocr
from rapidocr.ch_ppocr_rec import TextRecInput, TextRecognizer
from rapidocr.utils.parse_parameters import ParseParams

PINYIN_TOP_CROP_RATIO = 0.39  # 每个固定高度条带默认覆盖顶部 39%，清除残留拼音。
# 固定高度方案的切分参数固定在代码中，避免调用端传入互相矛盾的尺寸配置。
FIXED_BLOCK_HEIGHT = 38
FIXED_GAP = 0
FIXED_TOP_PAD = 5
FIXED_BOTTOM_PAD = 0
# 水印主体颜色 #FFD5CC 的 RGB 值。水印先经轻度模糊后颜色稳定，因此使用较小容差
# 定位，尽量不将正文边缘和其他浅色内容包含进掩码。
WATERMARK_RGB = np.array([255, 213, 204], dtype=np.int16)
# RGB 颜色距离平方阈值，允许的最大颜色偏差。
WATERMARK_COLOR_TOLERANCE = 25
# 水印掩码只处理亮色区域，避免误伤深色正文或其他插图颜色。该值为 RGB 三通道平均值。
WATERMARK_MIN_BRIGHTNESS = 150
# 水印掩码在检测到的水印边缘外扩张一圈像素，补上抗锯齿边缘。
WATERMARK_MASK_EXPANSION = 1
def flatten_frame(frame: Image.Image) -> Image.Image:
    """
    将 GIF 帧转换为不含透明通道的 RGB 图像，便于后续灰度化和 OCR。
    """
    # GIF 帧可能带有透明通道；先统一转换为 RGBA，才能正确取得透明蒙版。
    rgba = frame.convert("RGBA")
    # 小说正文背景按白色处理，避免透明像素在灰度化后被误判为笔画。
    output = Image.new("RGB", rgba.size, "white")
    # 仅将源图的非透明像素贴到白底，得到可直接 OCR 的不透明 RGB 图像。
    output.paste(rgba, mask=rgba.getchannel("A"))

    return output


def denoise_frame(image: Image.Image) -> Image.Image:
    """生成仅用于水印定位的轻度模糊副本。"""
    # 半径 0.6 的高斯模糊只平滑孤立杂色和水印颜色波动，不直接参与 OCR。
    # 不在这里锐化：锐化会重新增强水印轮廓和噪声，令水印掩码边缘更明显。
    return image.filter(ImageFilter.GaussianBlur(radius=0.6))


def watermark_mask(image: Image.Image) -> np.ndarray:
    """从去噪副本中检测接近 #FFD5CC 的背景水印位置。"""
    # 转为 32 位整数，避免 uint8 相减或 int16 平方时发生溢出，导致颜色距离计算错误。
    rgb = np.asarray(image.convert("RGB"), dtype=np.int32)
    # 使用平方距离避免逐像素开方；与欧氏距离阈值的比较结果完全等价。
    color_distance_squared = ((rgb - WATERMARK_RGB) ** 2).sum(axis=2)
    # 额外限制亮度与颜色方向：只处理浅色玫瑰红，避免误伤深色正文或其他插图颜色。
    light = rgb.mean(axis=2) >= WATERMARK_MIN_BRIGHTNESS
    pink = (rgb[:, :, 0] >= rgb[:, :, 1] + 8) & (rgb[:, :, 1] >= rgb[:, :, 2] + 2)
    mask = (color_distance_squared <= WATERMARK_COLOR_TOLERANCE**2) & light & pink
    if WATERMARK_MASK_EXPANSION:
        # 仅扩张一圈像素，补上水印抗锯齿外缘；后续 apply_watermark_mask() 仍会保护深色文字。
        mask_image = Image.fromarray((mask * 255).astype(np.uint8), mode="L")
        mask = (
            np.asarray(
                mask_image.filter(
                    ImageFilter.MaxFilter(size=WATERMARK_MASK_EXPANSION * 2 + 1)
                )
            )
            > 0
        )
    return mask


def apply_watermark_mask(image: Image.Image, mask: np.ndarray) -> Image.Image:
    """将降噪副本检测到的水印位置覆盖到未去噪的原图。"""
    # 原图始终保留给 OCR，以避免中值滤波改变汉字边缘和细小笔画。
    rgb = np.asarray(image.convert("RGB"), dtype=np.uint8).copy()
    if mask.shape != rgb.shape[:2]:
        raise ValueError("水印掩码尺寸与原图不一致")
    # 页面底色已在 flatten_frame() 中统一为白色；亮色水印区域填白即可恢复背景。
    brightness = rgb.mean(axis=2)
    light_watermark = mask & (brightness >= 185)
    rgb[light_watermark] = 255
    # 水印与深色文字重叠时不能填白，否则会把文字笔画一起抹掉。
    # 将这些命中像素转为等值灰度，去掉玫瑰红色偏移，同时保留其深色笔画强度。
    dark_overlap = mask & ~light_watermark
    if dark_overlap.any():
        gray = (
            np.rint(rgb[:, :, 0] * 0.299 + rgb[:, :, 1] * 0.587 + rgb[:, :, 2] * 0.114)
            .clip(0, 255)
            .astype(np.uint8)
        )
        rgb[dark_overlap] = gray[dark_overlap, None]
    return Image.fromarray(rgb, mode="RGB")


def frames_from_file(path: Path) -> list[Image.Image]:
    """
    将输入图片按帧读取为 Pillow 图像列表，便于逐帧预处理、分条和识别。
    """

    # Pillow 根据文件内容识别 PNG、GIF 等格式，并在退出上下文时关闭文件句柄。
    with Image.open(path) as source:
        # 静态图片只有一帧，GIF 则按原始顺序返回所有帧；copy() 保证关闭文件后数据仍有效。
        return [flatten_frame(frame.copy()) for frame in ImageSequence.Iterator(source)]


def has_recognizable_content(image: Image.Image) -> bool:
    """判断图像是否包含足以交给 OCR 的非白色像素。"""
    # 使用与裁白边相同的 250 阈值，将纯白和压缩产生的极浅背景视为无内容。
    gray = np.asarray(image.convert("L"))
    return bool((gray < 250).any())


class RecognitionEngine:
    """仅使用外置 PP-OCRv6 识别模型的 RapidOCR 执行器。"""

    def __init__(self, model_path: Path):
        if not model_path.is_file():
            raise RuntimeError("OCR 识别模型不存在；请按 README.md 中的说明下载 PP-OCRv6_rec_small.onnx")
        config_path = Path(rapidocr.__file__).with_name("config.yaml")
        config = ParseParams.load(config_path)
        # 正文图片已按固定高度分成正向单行，不加载检测或方向分类模型。
        config.Rec.model_path = str(model_path)
        config.Rec.engine_cfg = config.EngineConfig[config.Rec.engine_type.value]
        config.Rec.font_path = config.Global.font_path
        config.Rec.model_root_dir = config.Global.model_root_dir
        self.engine = TextRecognizer(config.Rec)

    def recognize(self, image: np.ndarray) -> str:
        result = self.engine(TextRecInput(img=image, return_word_box=False))
        return result.txts[0] if result.txts else ""


def create_engine(model_path: Path) -> RecognitionEngine:
    """创建单模型识别引擎，不初始化检测或方向分类器。"""
    return RecognitionEngine(model_path)


def recognition_only(engine, line: Image.Image) -> str:
    """
    仅对单行图像执行 OCR 识别，返回汉字文本。"""
    value = engine.recognize(np.asarray(line.convert("RGB"))).strip()
    return value if any("\u4e00" <= char <= "\u9fff" for char in value) else ""


def save_segment_preview(
    image: Image.Image,
    segments_dir: Path | None,
    frame_index: int,
    suffix: str,
) -> None:
    # 未要求诊断输出时不创建中间文件，正常下载路径仅返回最终文本。
    if segments_dir is None:
        return
    # 按需创建调用方传入的章节诊断目录。
    segments_dir.mkdir(parents=True, exist_ok=True)
    # 文件名包含帧序号与处理阶段，方便按 cropped、stripped、line 的顺序检查。
    image.save(segments_dir / f"frame-{frame_index:04d}-{suffix}.png")


def prepare_frame(
    frame: Image.Image,
    segments_dir: Path | None,
    frame_index: int,
) -> Image.Image | None:
    """完成单帧去噪、水印清理和纯白检查。"""
    # 去噪副本只用于定位水印，OCR 始终使用保留原始文字细节的 frame。
    denoised = denoise_frame(frame)
    save_segment_preview(denoised, segments_dir, frame_index, "denoised")
    mask = watermark_mask(denoised)
    # 白色为掩码命中位置，黑色为保留位置，便于检查水印边缘是否覆盖正文。
    save_segment_preview(
        Image.fromarray(np.where(mask, 255, 0).astype(np.uint8)),
        segments_dir,
        frame_index,
        "watermark-mask",
    )
    cleaned = apply_watermark_mask(frame, mask)
    save_segment_preview(cleaned, segments_dir, frame_index, "watermark-removed")
    # 水印清除后仍是纯白的帧不进入灰度化、分隔和 OCR。
    return cleaned if has_recognizable_content(cleaned) else None


def grayscale_frame(image: Image.Image) -> np.ndarray:
    """将已清除水印的原图转换为灰度数组。"""
    # 延迟到水印处理之后灰度化，避免丢失用于识别 #FFD5CC 的颜色信息。
    return np.asarray(image.convert("L"))


def split_fixed_height(gray: np.ndarray) -> list[tuple[int, int]]:
    """按代码内固定参数生成不包含间隔区的条带边界。"""
    # 这些值对应 test.py：顶部忽略 5 像素，每条 38 像素，条带间隔和底部忽略为 0。
    if FIXED_BLOCK_HEIGHT < 1 or FIXED_GAP < 0 or FIXED_TOP_PAD < 0:
        raise ValueError("固定高度分行参数无效")
    if FIXED_TOP_PAD + FIXED_BOTTOM_PAD >= gray.shape[0]:
        raise ValueError("固定高度分行的边缘忽略高度不能覆盖整张图片")
    bounds: list[tuple[int, int]] = []
    start = FIXED_TOP_PAD
    content_end = gray.shape[0] - FIXED_BOTTOM_PAD
    while start < content_end:
        # 最后一条允许不足固定高度，保留图片底部剩余内容。
        end = min(start + FIXED_BLOCK_HEIGHT, content_end)
        bounds.append((start, end))
        start = end + FIXED_GAP
    return bounds


def cover_pinyin(
    gray: np.ndarray, bounds: list[tuple[int, int]], ratio: float
) -> np.ndarray:
    """用白色矩形覆盖每个固定条带顶部的拼音区域。"""
    # 在副本上修改，保持原始灰度数组不变，并维持每个条带的固定高度。
    result = gray.copy()
    for start, end in bounds:
        cover_end = start + round((end - start) * ratio)
        # 只覆盖条带顶部，绝不通过裁剪改变正文的纵向坐标。
        result[start:cover_end, :] = 255
    return result


def extract_fixed_height_lines(
    gray: np.ndarray,
    bounds: list[tuple[int, int]],
) -> list[Image.Image]:
    """按边界提取完整宽度的固定高度条带。"""
    # 固定高度方案保留整幅图片宽度，不执行投影分行或左右白边裁剪。
    return [Image.fromarray(gray[start:end, :]) for start, end in bounds]


def process_frame(
    frame: Image.Image,
    segments_dir: Path | None,
    frame_index: int,
    pinyin_top_crop_ratio: float,
) -> list[Image.Image]:
    """执行单帧完整预处理并返回非纯白 OCR 条带。"""
    prepared = prepare_frame(frame, segments_dir, frame_index)
    if prepared is None:
        return []
    gray = grayscale_frame(prepared)
    bounds = split_fixed_height(gray)
    covered = cover_pinyin(gray, bounds, pinyin_top_crop_ratio)
    # 保存整张覆盖结果，方便检查固定条带顶部是否覆盖过多或过少。
    save_segment_preview(
        Image.fromarray(covered), segments_dir, frame_index, "pinyin-covered"
    )
    lines = extract_fixed_height_lines(covered, bounds)
    content_lines = [line for line in lines if has_recognizable_content(line)]
    for line_index, line in enumerate(content_lines, start=1):
        save_segment_preview(line, segments_dir, frame_index, f"line-{line_index:04d}")
    return content_lines


def recognize(
    path: Path,
    model_path: Path,
    workers: int,
    segments_dir: Path | None,
) -> str:
    line_images: list[Image.Image] = []
    # 静态图片仅产生一个帧，GIF 依原始帧顺序分别预处理并保持输出顺序。
    for frame_index, frame in enumerate(frames_from_file(path), start=1):
        line_images.extend(
            process_frame(frame, segments_dir, frame_index, PINYIN_TOP_CROP_RATIO)
        )
    # 没有有效条带时不加载 OCR 模型，直接返回空文本。
    if not line_images:
        return ""
    # 单线程模式只加载一个引擎，避免重复初始化 ONNX 模型。
    engine = create_engine(model_path)
    # ONNX Runtime 会话默认复用；显式并发时每项独占一个会话，避免跨线程共享状态。
    if workers <= 1:
        # 过滤空结果后逐行拼接，保留 line_images 中的原始阅读顺序。
        return "\n".join(
            filter(None, (recognition_only(engine, line) for line in line_images))
        )

    def one(index: int, image: Image.Image) -> tuple[int, str]:
        # 返回原始索引，因为并发任务完成顺序不等于图片中的行序。
        return index, recognition_only(create_engine(model_path), image)

    results: dict[int, str] = {}
    # main() 已将 workers 限制为 1 至 4，避免并发加载过多 ONNX Runtime 模型。
    with ThreadPoolExecutor(max_workers=workers) as pool:
        # 为每个行图分配稳定序号，稍后按该序号恢复正常阅读顺序。
        futures = [
            pool.submit(one, index, line) for index, line in enumerate(line_images)
        ]
        for future in as_completed(futures):
            # result() 会重新抛出子线程异常，使 worker 正确写 stderr 并返回失败退出码。
            index, text = future.result()
            if text:
                # 仅保留非空识别结果，但不丢失其源图位置。
                results[index] = text
    # 将乱序完成的结果按原始行序排序，每个图片行输出为一行文本。
    return "\n".join(results[index] for index in sorted(results))


def main() -> int:
    # 参数由 Rust OCR 调用层传入；worker 只处理已落地的本地图片，不访问网络或账号数据。
    parser = argparse.ArgumentParser()
    # 使用 Path 类型，令后续文件校验和 Pillow 打开都操作同一个路径对象。
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--model-path", required=True, type=Path)
    # 可选中间图输出目录，供人工核查水印掩码、拼音覆盖和固定高度条带。
    parser.add_argument("--segments-dir", type=Path)
    # workers 只影响逐行 OCR 的并发度；图像帧的预处理始终按顺序执行。
    parser.add_argument("--workers", default=1, type=int)
    args = parser.parse_args()
    # 先验证输入存在，避免在加载模型后才将路径问题表现成 OCR 错误。
    if not args.input.is_file():
        raise RuntimeError("OCR 输入图片不存在")
    if not args.model_path.is_file():
        raise RuntimeError("OCR 识别模型不存在；请按 README.md 中的说明下载 PP-OCRv6_rec_small.onnx")
    # 预先保存 stdout 的二进制协议流。redirect_stdout 会替换 sys.stdout；若之后通过
    # sys.stdout.buffer 输出结果，成功文本会误写到 stderr，Rust 将只能收到空输出。
    protocol_stdout = sys.stdout.buffer
    # stdout 是 Rust 约定的唯一成功结果通道，运行时日志全部送入 stderr。
    with contextlib.redirect_stdout(sys.stderr):
        # 将用户给出的并发数夹在 1 到 4 内，防止零/负值及过多模型实例占用内存。
        text = recognize(
            args.input,
            args.model_path,
            max(1, min(args.workers, 4)),
            args.segments_dir,
        )
    # 协议只允许 stdout 输出 UTF-8 正文，不混入诊断信息、JSON 或额外包装。
    protocol_stdout.write(text.encode("utf-8"))
    # 显式刷新，保证 Rust 在子进程退出前读取完整 OCR 结果。
    protocol_stdout.flush()
    return 0


if __name__ == "__main__":
    try:
        # 退出码 0 表示 worker 成功完成；调用端据此区分正常空输出和执行异常。
        raise SystemExit(main())
    except Exception as error:
        # 未处理异常只写 stderr，保留异常类型和消息供 Rust 提取可诊断的失败原因。
        detail = str(error).strip() or "未提供详细信息"
        print(f"{type(error).__name__}: {detail}", file=sys.stderr)
        raise SystemExit(1)
