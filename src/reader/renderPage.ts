/**
 * 单页位图渲染器。
 *
 * 把分页阶段产出的绘制指令绘制到一张离屏 canvas 上。返回的位图会被
 * 翻页引擎缓存复用，翻页动画期间只做 `drawImage`，不再重绘文字。
 * 位图按设备像素比放大，保证高分屏下文字清晰。
 */

import type { ReaderPage, ReaderStyle, ReaderViewport } from "./types";

/**
 * 将一页内容渲染为离屏 canvas。
 * @param page 分页结果。
 * @param style 排版样式。
 * @param viewport 页面逻辑尺寸（CSS 像素）。
 * @param dpr 设备像素比（用于位图分辨率）。
 */
export function renderPage(
  page: ReaderPage,
  style: ReaderStyle,
  viewport: ReaderViewport,
  dpr: number,
): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(viewport.width * dpr));
  canvas.height = Math.max(1, Math.round(viewport.height * dpr));
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("无法创建页面渲染 Canvas 上下文");

  ctx.scale(dpr, dpr);
  ctx.fillStyle = style.background;
  ctx.fillRect(0, 0, viewport.width, viewport.height);
  ctx.textBaseline = "top";
  ctx.textAlign = "left";

  for (const item of page.items) {
    if (item.type === "text") {
      ctx.font = item.font;
      ctx.fillStyle = item.color;
      ctx.fillText(item.text, item.x, item.y);
    } else {
      ctx.drawImage(item.image, item.x, item.y, item.width, item.height);
    }
  }

  return canvas;
}
