/**
 * 分页阅读器的公共类型定义。
 *
 * 该模块只描述数据形状，不包含任何运行时逻辑，便于分页、渲染、
 * 翻页引擎与 Vue 组合式函数之间共享类型。
 */

/** 章节正文的原始片段：一段文本或一张插图。 */
export type ReaderChapterPart =
  | { type: "text"; value: string }
  | { type: "image"; alt: string; src: string };

/**
 * 画布分页使用的排版样式，所有尺寸单位均为 CSS 像素。
 * 与纵向滚动模式的 CSS 保持一致，保证两种模式观感统一。
 */
export interface ReaderStyle {
  /** 正文字体族。 */
  fontFamily: string;
  /** 正文字号。 */
  fontSize: number;
  /** 行高倍数（实际行高 = fontSize * lineHeight）。 */
  lineHeight: number;
  /** 正文颜色。 */
  color: string;
  /** 页面背景色。 */
  background: string;
  /** 正文区域四边内边距。 */
  padding: { top: number; right: number; bottom: number; left: number };
}

/** 页面逻辑尺寸（CSS 像素）。 */
export interface ReaderViewport {
  width: number;
  height: number;
}

/**
 * 页面中的绘制单元。所有坐标与字体信息在分页阶段已经计算完成，
 * 渲染阶段只做无测量的纯绘制，避免任何强制回流。
 */
export type ReaderPageItem =
  | {
      type: "text";
      text: string;
      /** 文本左边缘（页面坐标系）。 */
      x: number;
      /** 文本顶部（页面坐标系）。 */
      y: number;
      /** 完整 canvas font 字符串。 */
      font: string;
      color: string;
    }
  | {
      type: "image";
      image: HTMLImageElement;
      x: number;
      y: number;
      width: number;
      height: number;
    };

/** 已完成分页的单页数据。 */
export interface ReaderPage {
  items: ReaderPageItem[];
}
