/**
 * 章节正文拆分。
 *
 * 本地章节是 markdown（`![alt](imgs/…)`），在线章节是后端返回的纯文本加
 * `{{image:n}}` 占位符。两种来源最终都要变成阅读器要求的 `ReaderChapterPart[]`，
 * 供分页时逐段排版。
 *
 * 这一层只做切分，不做任何 IO：本地图片的可读地址由调用方通过 `resolveImage`
 * 提供，避免把 Tauri 的路径转换带进纯函数。
 */

import type { ReaderChapterPart } from "./types";

/** 本地章节的插图标记：`![说明](imgs/chapter-1-1.jpeg)`。 */
const LOCAL_IMAGE_PATTERN = /!\[([^\]]*)\]\((imgs[\\/][^)]+)\)/g;
/** 在线章节的插图占位符：`{{image:0}}`，序号对应 `images` 数组下标。 */
const ONLINE_IMAGE_PATTERN = /\{\{image:(\d+)\}\}/g;
/** 正文开头由导出流程写入的章标题，阅读页已有标题栏，不再重复显示。 */
const LEADING_TITLE_PATTERN = /^##\s+.+\r?\n+/;

/**
 * 拆分本地章节正文。
 *
 * # 参数
 * - `rawContent`：章节文件内容，图片以 markdown 相对路径引用。
 * - `resolveImage`：把 `imgs/xxx` 转成渲染进程可加载的地址。
 *
 * # 返回
 * 按出现顺序排列的文本段与插图段。首尾的纯文本段不会被丢弃，
 * 但空文本段不会产生。
 */
export function parseLocalChapterParts(
  rawContent: string,
  resolveImage: (relativePath: string) => string,
): ReaderChapterPart[] {
  const content = rawContent.replace(LEADING_TITLE_PATTERN, "");
  const parts: ReaderChapterPart[] = [];
  let lastIndex = 0;

  for (const match of content.matchAll(LOCAL_IMAGE_PATTERN)) {
    const index = match.index ?? 0;
    if (index > lastIndex)
      parts.push({ type: "text", value: content.slice(lastIndex, index) });
    parts.push({
      type: "image",
      alt: match[1] || "章节插图",
      src: resolveImage(match[2]),
    });
    lastIndex = index + match[0].length;
  }

  if (lastIndex < content.length)
    parts.push({ type: "text", value: content.slice(lastIndex) });
  return parts;
}

/**
 * 拆分离线章节正文。
 *
 * 在线正文的图片已经被后端下载并改写为占位符，这里只负责按占位符切开，
 * 文本段会 trim——在线正文以换行拼接，直接排版会出现多余行距。
 *
 * # 参数
 * - `payload.content`：带 `{{image:n}}` 占位符的正文。
 * - `payload.images`：占位符序号对应的可读图片地址。
 * - `payload.title`：章节标题，用于生成插图说明。
 *
 * # 返回
 * 文本段与插图段交替排列的序列；占位符指向不存在的图片时跳过该插图。
 */
export function parseOnlineChapterParts(payload: {
  content: string;
  images: string[];
  title: string;
}): ReaderChapterPart[] {
  const parts: ReaderChapterPart[] = [];
  let lastIndex = 0;

  for (const match of payload.content.matchAll(ONLINE_IMAGE_PATTERN)) {
    const index = match.index ?? 0;
    const text = payload.content.slice(lastIndex, index).trim();
    if (text) parts.push({ type: "text", value: text });
    const src = payload.images[Number(match[1])];
    if (src) parts.push({ type: "image", alt: `${payload.title} 插图`, src });
    lastIndex = index + match[0].length;
  }

  const tail = payload.content.slice(lastIndex).trim();
  if (tail) parts.push({ type: "text", value: tail });
  return parts;
}
