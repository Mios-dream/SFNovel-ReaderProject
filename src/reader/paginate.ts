/**
 * 章节分页器。
 *
 * 将一个章节的正文片段（文本段 + 插图）按给定的视口尺寸拆分为若干页。
 * 分页过程只依赖文本测量，产出带绝对坐标的绘制指令，渲染阶段无需再测量。
 */

import { measureContext, wrapParagraph, wrapLine } from "./textLayout";
import type {
  ReaderChapterPart,
  ReaderPage,
  ReaderPageItem,
  ReaderStyle,
  ReaderViewport,
} from "./types";

/** 分页输入参数。 */
export interface PaginateOptions {
  /** 章节正文片段。 */
  parts: ReaderChapterPart[];
  /** 章节标题。 */
  title: string;
  /** 排版样式。 */
  style: ReaderStyle;
  /** 页面尺寸。 */
  viewport: ReaderViewport;
  /** 已解码的插图，按 src 索引。 */
  images: Map<string, HTMLImageElement>;
  /** 插图在单页内允许占用的最大高度比例，默认 0.72。 */
  maxImageHeightRatio?: number;
}

/** 标题字号相对正文字号的倍数。 */
const HEADING_FONT_RATIO = 1.6;
/** 标题行高倍数（与 CSS 的 line-height: 1.35 对齐）。 */
const HEADING_LINE_HEIGHT_RATIO = 1.35;
/** 标题与正文之间的间距（正文字号的倍数）。 */
const HEADING_MARGIN_RATIO = 1.1;
/** 插图上下留白（正文字号的倍数）。 */
const IMAGE_MARGIN_RATIO = 1;
/** 浮点误差容忍值，避免正好贴边时被误判为溢出。 */
const EPSILON = 1;

/**
 * 将章节内容分页。
 * @returns 至少包含一页；每页的绘制单元坐标均为页面绝对坐标。
 */
export function paginateText(options: PaginateOptions): ReaderPage[] {
  const { parts, title, style, viewport, images } = options;
  const ctx = measureContext();
  const bodyFont = `${style.fontSize}px ${style.fontFamily}`;
  const headingFont = `bold ${Math.round(style.fontSize * HEADING_FONT_RATIO)}px ${style.fontFamily}`;
  const lineHeight = style.fontSize * style.lineHeight;
  const headingLineHeight =
    Math.round(style.fontSize * HEADING_FONT_RATIO) *
    HEADING_LINE_HEIGHT_RATIO;
  const headingMargin = style.fontSize * HEADING_MARGIN_RATIO;
  const imageMargin = style.fontSize * IMAGE_MARGIN_RATIO;
  const { padding } = style;
  const contentWidth = Math.max(1, viewport.width - padding.left - padding.right);
  const contentHeight = Math.max(
    1,
    viewport.height - padding.top - padding.bottom,
  );

  const pages: ReaderPage[] = [];
  let items: ReaderPageItem[] = [];
  // 当前页已占用的内容高度（相对内容区顶部）。
  let cursor = 0;

  /** 提交当前页并开启新页。 */
  const flushPage = () => {
    pages.push({ items });
    items = [];
    cursor = 0;
  };

  /** 确保当前页还能容纳指定高度，否则换页。 */
  const ensureRoom = (required: number) => {
    if (cursor + required > contentHeight + EPSILON && items.length > 0) {
      flushPage();
    }
  };

  // 标题优先占据页面顶部。
  const headingLines = wrapLine(ctx, title, contentWidth, headingFont);
  for (const line of headingLines) {
    ensureRoom(headingLineHeight);
    items.push({
      type: "text",
      text: line,
      x: padding.left,
      y: padding.top + cursor,
      font: headingFont,
      color: style.color,
    });
    cursor += headingLineHeight;
  }
  cursor += headingMargin;

  for (const part of parts) {
    if (part.type === "text") {
      const lines = wrapParagraph(ctx, part.value, contentWidth, bodyFont);
      for (const line of lines) {
        ensureRoom(lineHeight);
        items.push({
          type: "text",
          text: line,
          x: padding.left,
          y: padding.top + cursor,
          font: bodyFont,
          color: style.color,
        });
        cursor += lineHeight;
      }
      continue;
    }

    const image = images.get(part.src);
    if (!image || !image.naturalWidth || !image.naturalHeight) continue;
    const ratio = options.maxImageHeightRatio ?? 0.72;
    const scale = Math.min(
      1,
      contentWidth / image.naturalWidth,
      (contentHeight * ratio) / image.naturalHeight,
    );
    const width = image.naturalWidth * scale;
    const height = image.naturalHeight * scale;
    const blockHeight = height + imageMargin;
    ensureRoom(blockHeight);
    items.push({
      type: "image",
      image,
      x: padding.left + (contentWidth - width) / 2,
      y: padding.top + cursor + imageMargin / 2,
      width,
      height,
    });
    cursor += blockHeight;
  }

  // 收尾：仅在当前页有内容或尚未产生任何页时提交，避免多余空白页。
  if (items.length > 0 || pages.length === 0) pages.push({ items });
  return pages;
}
