"""Offline OCR worker for authorized SFACG chapter images.

It consumes only local image paths. Network access, cookies, account state, and
chapter selection remain in the Tauri Rust process. The implementation follows
the useful parts of light-nook-labs/sfacg_tools: GIF frame flattening, whitespace
cropping, rule-based line segmentation, pinyin removal, and recognition-only OCR.
"""

from __future__ import annotations

import argparse
import contextlib
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

import numpy as np
from PIL import Image, ImageSequence

DEFAULT_PINYIN_TOP_CROP_RATIO = 0.30
# RapidOCR 默认会将超出 2000 像素的边缩小到 32 的倍数。纯识别模式中的文字行可能
# 极宽但很矮，缩放后高度会被舍入为 0；提高限制后由最小边逻辑安全放大短边。
RECOGNITION_MAX_SIDE_LEN = 10_000


def flatten_frame(frame: Image.Image) -> Image.Image:
    rgba = frame.convert("RGBA")
    output = Image.new("RGB", rgba.size, "white")
    output.paste(rgba, mask=rgba.getchannel("A"))
    return output


def frames_from_file(path: Path) -> list[Image.Image]:
    with Image.open(path) as source:
        return [flatten_frame(frame.copy()) for frame in ImageSequence.Iterator(source)]


def crop_whitespace(image: Image.Image) -> Image.Image | None:
    gray = np.asarray(image.convert("L"))
    rows = np.where((gray < 250).any(axis=1))[0]
    columns = np.where((gray < 250).any(axis=0))[0]
    if not len(rows) or not len(columns):
        return None
    return image.crop((columns[0], rows[0], columns[-1] + 1, rows[-1] + 1))


def line_bounds(gray: np.ndarray) -> list[tuple[int, int]]:
    occupied = (gray < 245).any(axis=1)
    bounds: list[tuple[int, int]] = []
    start: int | None = None
    for index, has_ink in enumerate(occupied):
        if has_ink and start is None:
            start = index
        if start is not None and (not has_ink or index == len(occupied) - 1):
            end = index if not has_ink else index + 1
            if end - start >= 10:
                bounds.append((start, end))
            start = None
    return bounds


def strip_pinyin(gray: np.ndarray, bounds: list[tuple[int, int]]) -> np.ndarray:
    result = gray.copy()
    for start, end in bounds:
        row_density = (result[start:end] < 190).sum(axis=1)
        if len(row_density) < 8:
            continue
        # Pinyin is normally the sparse upper strip of a text line. Preserve it
        # when the expected lower body is absent so unusual typography is not
        # erased wholesale.
        split = start + max(1, (end - start) // 3)
        upper = row_density[: split - start].mean()
        lower = row_density[split - start :].mean()
        if upper > 0 and lower > upper * 1.35:
            result[start:split, :] = 255
    return result


def crop_lines(
    gray: np.ndarray,
    bounds: list[tuple[int, int]],
    pinyin_top_crop_ratio: float,
) -> list[Image.Image]:
    lines: list[Image.Image] = []
    for start, end in bounds:
        top = start + round((end - start) * pinyin_top_crop_ratio)
        segment = gray[top:end, :]
        columns = np.where((segment < 245).any(axis=0))[0]
        if len(columns):
            lines.append(Image.fromarray(segment[:, columns[0] : columns[-1] + 1]))
    return lines


def create_engine():
    try:
        from rapidocr_onnxruntime import RapidOCR
    except ImportError as error:
        raise RuntimeError(
            "缺少 rapidocr_onnxruntime；请先运行 uv sync --project src-tauri/ocr-worker"
        ) from error
    return RapidOCR(max_side_len=RECOGNITION_MAX_SIDE_LEN)


def recognition_only(engine, line: Image.Image) -> str:
    result = engine(np.asarray(line), use_det=False, use_cls=False, use_rec=True)
    records = result[0] if isinstance(result, tuple) else result
    text: list[str] = []
    for record in records or []:
        if isinstance(record, (list, tuple)) and record:
            value = str(record[0]).strip()
        else:
            value = str(record).strip()
        if any("\u4e00" <= char <= "\u9fff" for char in value):
            text.append(value)
    return "".join(text)


def save_segment_preview(
    image: Image.Image,
    segments_dir: Path | None,
    frame_index: int,
    suffix: str,
) -> None:
    if segments_dir is None:
        return
    segments_dir.mkdir(parents=True, exist_ok=True)
    image.save(segments_dir / f"frame-{frame_index:04d}-{suffix}.png")


def recognize(
    path: Path,
    workers: int,
    segments_dir: Path | None,
    pinyin_top_crop_ratio: float,
) -> str:
    line_images: list[Image.Image] = []
    for frame_index, frame in enumerate(frames_from_file(path), start=1):
        cropped = crop_whitespace(frame)
        if cropped is None:
            continue
        save_segment_preview(cropped, segments_dir, frame_index, "cropped")
        gray = np.asarray(cropped.convert("L"))
        bounds = line_bounds(gray)
        stripped = strip_pinyin(gray, bounds)
        save_segment_preview(Image.fromarray(stripped), segments_dir, frame_index, "stripped")
        lines = crop_lines(stripped, bounds, pinyin_top_crop_ratio)
        for line_index, line in enumerate(lines, start=1):
            save_segment_preview(line, segments_dir, frame_index, f"line-{line_index:04d}")
        line_images.extend(lines)
    if not line_images:
        return ""
    engine = create_engine()
    # RapidOCR instances are not documented as thread-safe. Each work item owns
    # one engine when parallel mode is explicitly requested.
    if workers <= 1:
        return "\n".join(filter(None, (recognition_only(engine, line) for line in line_images)))
    def one(index: int, image: Image.Image) -> tuple[int, str]:
        return index, recognition_only(create_engine(), image)
    results: dict[int, str] = {}
    with ThreadPoolExecutor(max_workers=workers) as pool:
        futures = [pool.submit(one, index, line) for index, line in enumerate(line_images)]
        for future in as_completed(futures):
            index, text = future.result()
            if text:
                results[index] = text
    return "\n".join(results[index] for index in sorted(results))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--segments-dir", type=Path)
    parser.add_argument(
        "--pinyin-top-crop-ratio",
        default=DEFAULT_PINYIN_TOP_CROP_RATIO,
        type=float,
        help="从每个分割行顶部裁剪的比例（0 到 0.9）",
    )
    parser.add_argument("--workers", default=1, type=int)
    args = parser.parse_args()
    if not args.input.is_file():
        raise RuntimeError("OCR 输入图片不存在")
    if not 0 <= args.pinyin_top_crop_ratio < 0.9:
        raise RuntimeError("--pinyin-top-crop-ratio 必须在 0 到 0.9 之间")
    # Preserve the protocol stream before redirecting dependency logs. `redirect_stdout`
    # changes sys.stdout itself, so writing through sys.stdout.buffer afterward would send
    # successful OCR text to stderr and Rust would incorrectly receive an empty result.
    protocol_stdout = sys.stdout.buffer
    # stdout is the Rust worker protocol; dependency diagnostics belong on stderr.
    with contextlib.redirect_stdout(sys.stderr):
        text = recognize(
            args.input,
            max(1, min(args.workers, 4)),
            args.segments_dir,
            args.pinyin_top_crop_ratio,
        )
    protocol_stdout.write(text.encode("utf-8"))
    protocol_stdout.flush()
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        detail = str(error).strip() or "未提供详细信息"
        print(f"{type(error).__name__}: {detail}", file=sys.stderr)
        raise SystemExit(1)
