/**
 * 封面取色。
 *
 * 首页 hero 的背景色跟随当前作品封面，做法是把封面缩到 32×32 后统计像素。
 * 这里只放算法本身：加载、缩放、聚类、加权平均。调用方负责缓存与竞态判断，
 * 因为「哪张封面才是当前封面」属于视图状态，不是算法的一部分。
 */

/** 缩略图边长。越小越快，32 已足够区分主要色块。 */
const SAMPLE_SIZE = 32;
/** 低于此 alpha 的像素是圆角或遮罩边缘，不参与统计。 */
const MIN_ALPHA = 180;
/** 接近纯黑/纯白的像素会让背景显得脏，跳过。 */
const MIN_LIGHTNESS = 28;
const MAX_LIGHTNESS = 238;
/** 每个通道保留高 4 位，把 24 位色空间压成 4096 个桶。 */
const CHANNEL_BITS = 4;
const CHANNEL_SHIFT = 8 - CHANNEL_BITS;

type Bucket = { red: number; green: number; blue: number; weight: number };

/**
 * 从封面图片提取主色调。
 *
 * # 返回
 * `rgb(r g b)` 形式的 CSS 颜色；图片加载失败、无法创建 canvas 上下文
 * 或没有合格像素时返回 `undefined`，由调用方决定回退色。
 */
export async function extractDominantColor(
  source: string,
): Promise<string | undefined> {
  const image = await loadImage(source);
  if (!image) return undefined;

  const context = createSampleContext();
  if (!context) return undefined;

  try {
    context.drawImage(image, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
    const { data } = context.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
    const bucket = dominantBucket(data);
    if (!bucket) return undefined;
    return `rgb(${Math.round(bucket.red / bucket.weight)} ${Math.round(
      bucket.green / bucket.weight,
    )} ${Math.round(bucket.blue / bucket.weight)})`;
  } catch {
    // 图片跨域且未放行 CORS 时 getImageData 会抛安全异常。
    return undefined;
  }
}

function loadImage(source: string): Promise<HTMLImageElement | undefined> {
  return new Promise((resolve) => {
    const image = new Image();
    image.crossOrigin = "anonymous";
    image.decoding = "async";
    image.onload = () => resolve(image);
    image.onerror = () => resolve(undefined);
    image.src = source;
  });
}

function createSampleContext(): CanvasRenderingContext2D | null {
  const canvas = document.createElement("canvas");
  canvas.width = SAMPLE_SIZE;
  canvas.height = SAMPLE_SIZE;
  return canvas.getContext("2d", { willReadFrequently: true });
}

/**
 * 按「饱和度加权的出现次数」选主色。
 *
 * 权重取 `12 + (最亮通道 - 最暗通道)`：灰色权重恒定，彩色随饱和度上升，
 * 因此画面里的彩色区域不会被大片背景色淹没。
 */
function dominantBucket(pixels: Uint8ClampedArray): Bucket | undefined {
  const buckets = new Map<number, Bucket>();

  for (let index = 0; index < pixels.length; index += 4) {
    if (pixels[index + 3] < MIN_ALPHA) continue;

    const red = pixels[index];
    const green = pixels[index + 1];
    const blue = pixels[index + 2];
    const high = Math.max(red, green, blue);
    const low = Math.min(red, green, blue);
    const lightness = (high + low) / 2;
    if (lightness < MIN_LIGHTNESS || lightness > MAX_LIGHTNESS) continue;

    const key =
      ((red >> CHANNEL_SHIFT) << (CHANNEL_BITS * 2)) |
      ((green >> CHANNEL_SHIFT) << CHANNEL_BITS) |
      (blue >> CHANNEL_SHIFT);
    const bucket = buckets.get(key) ?? {
      red: 0,
      green: 0,
      blue: 0,
      weight: 0,
    };
    const weight = 12 + high - low;
    bucket.red += red * weight;
    bucket.green += green * weight;
    bucket.blue += blue * weight;
    bucket.weight += weight;
    buckets.set(key, bucket);
  }

  let best: Bucket | undefined;
  for (const bucket of buckets.values()) {
    if (!best || bucket.weight > best.weight) best = bucket;
  }
  return best;
}
