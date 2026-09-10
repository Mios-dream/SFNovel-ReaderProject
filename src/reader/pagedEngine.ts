/**
 * 分页翻页引擎。
 *
 * 负责把章节内容分页、渲染为页面位图，并在 canvas 上以水平平移的方式
 * 完成翻页动画。设计要点：
 * - 页面位图按需渲染并做 LRU 缓存，翻页过程只 `drawImage`，不重排文字；
 * - 连续位置模型（position 为浮点页索引），拖拽、惯性、回弹统一处理；
 * - 手势与动画自带节流：仅在需要时请求 `requestAnimationFrame`；
 * - 与框架无关，通过回调向外部同步页码与边界事件。
 */

import { paginateText } from "./paginate";
import { renderPage } from "./renderPage";
import type {
  ReaderChapterPart,
  ReaderPage,
  ReaderStyle,
  ReaderViewport,
} from "./types";

/** 引擎向外抛出的回调。 */
export interface PagedEngineCallbacks {
  /** 稳定页发生变化时触发，页索引从 0 开始。 */
  onPageChange?: (index: number, total: number) => void;
  /** 用户请求进入下一章（在最后一页继续前进）。 */
  onReachEnd?: () => void;
  /** 用户请求返回上一章（在第一页继续后退）。 */
  onReachStart?: () => void;
  /** 点击左/中/右三个区域。 */
  onTap?: (zone: "prev" | "center" | "next") => void;
}

/** 拖拽超过该页宽比例即判定为翻页意图。 */
const FLIP_RATIO = 0.22;
/** 速度阈值（页/毫秒），超过则按惯性翻页。 */
const FLIP_VELOCITY = 0.35 / 1000;
/** 越界拖拽的最大橡皮筋位移（页比例）。 */
const RUBBER_RATIO = 0.28;
/** 判定主方向的位移阈值（像素）。 */
const AXIS_LOCK = 6;
/** 点击允许的最大时长（毫秒）。 */
const TAP_TIME = 260;
/** 位图缓存上限，超过后优先淘汰离当前页最远的。 */
const MAX_BITMAPS = 6;
/** 翻页动画默认时长（毫秒）。 */
const FLIP_DURATION = 300;

/** 缓出曲线，让翻页收尾更自然。 */
function easeOutCubic(t: number): number {
  return 1 - Math.pow(1 - t, 3);
}

/** 加载并解码一张插图，失败时 reject。 */
function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.decoding = "async";
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error(`插图加载失败：${src}`));
    image.src = src;
  });
}

export class PagedEngine {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private readonly callbacks: PagedEngineCallbacks;

  private style: ReaderStyle | undefined;
  private viewport: ReaderViewport = { width: 0, height: 0 };
  private dpr = 1;

  private parts: ReaderChapterPart[] = [];
  private title = "";
  private readonly images = new Map<string, HTMLImageElement>();
  private readonly failedImages = new Set<string>();

  private pages: ReaderPage[] = [];
  private readonly bitmaps = new Map<number, HTMLCanvasElement>();

  /** 相邻章节的边界页位图：位置 -1 显示上一章末页，位置 count 显示下一章首页。 */
  private previousPreview?: HTMLCanvasElement;
  private nextPreview?: HTMLCanvasElement;
  /** 已滑到章节边界并等待上层切换章节时为真，期间忽略手势。 */
  private committing = false;

  /** 连续页位置：整数表示对齐某页，小数表示翻页中间态。 */
  private position = 0;
  private animation?: {
    from: number;
    to: number;
    start: number;
    duration: number;
  };
  private rafId = 0;
  /** 内容版本号，用于丢弃过期的异步分页结果。 */
  private revision = 0;
  private destroyed = false;

  private gesture?: {
    pointerId: number;
    startX: number;
    startY: number;
    startPosition: number;
    startTime: number;
    lastX: number;
    lastTime: number;
    /** 速度（页/毫秒，正值表示向下一页）。 */
    velocity: number;
    axis: "x" | "y" | null;
  };

  constructor(canvas: HTMLCanvasElement, callbacks: PagedEngineCallbacks) {
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("无法创建翻页 Canvas 上下文");
    this.canvas = canvas;
    this.ctx = ctx;
    this.callbacks = callbacks;
    this.canvas.style.touchAction = "none";
    this.canvas.addEventListener("pointerdown", this.handlePointerDown);
    this.canvas.addEventListener("pointermove", this.handlePointerMove);
    this.canvas.addEventListener("pointerup", this.handlePointerUp);
    this.canvas.addEventListener("pointercancel", this.handlePointerCancel);
  }

  /** 当前稳定页索引（从 0 开始）。 */
  get pageIndex(): number {
    const max = Math.max(0, this.pages.length - 1);
    return Math.min(Math.max(0, Math.round(this.position)), max);
  }

  /** 总页数。 */
  get pageCount(): number {
    return this.pages.length;
  }

  /** 更新排版样式与视口尺寸。 */
  configure(style: ReaderStyle, viewport: ReaderViewport): void {
    this.style = style;
    this.viewport = {
      width: Math.max(1, Math.floor(viewport.width)),
      height: Math.max(1, Math.floor(viewport.height)),
    };
    this.dpr = Math.min(window.devicePixelRatio || 1, 2);
  }

  /**
   * 设置相邻章节的边界页位图。传 undefined 表示该方向没有相邻章节。
   * 预渲染由上层调用 {@link buildPageBitmap} 完成。
   */
  setPreviews(previews: {
    previous?: HTMLCanvasElement;
    next?: HTMLCanvasElement;
  }): void {
    this.previousPreview = previews.previous;
    this.nextPreview = previews.next;
    this.render();
  }

  /**
   * 用当前排版参数渲染指定章节的边界页位图，供跨章平滑过渡使用。
   * @param edge `first` 取首页，`last` 取末页。
   * @returns 位图；样式未就绪或无内容时返回 undefined。
   */
  async buildPageBitmap(
    parts: ReaderChapterPart[],
    title: string,
    edge: "first" | "last",
  ): Promise<HTMLCanvasElement | undefined> {
    if (!this.style || !this.viewport.width) return undefined;
    await this.loadImages(parts);
    if (this.destroyed || !this.style) return undefined;
    const pages = paginateText({
      parts,
      title,
      style: this.style,
      viewport: this.viewport,
      images: this.images,
    });
    if (!pages.length) return undefined;
    const page = edge === "first" ? pages[0] : pages[pages.length - 1];
    return renderPage(page, this.style, this.viewport, this.dpr);
  }

  /**
   * 设置章节内容并重新分页。
   * @param options.atEnd 布局完成后定位到最后一页（用于上一章回退）。
   * @param options.preservePosition 布局完成后尽量保持当前页索引（用于尺寸变化）。
   * @param options.adopt 复用在边界滑动时已显示的相邻章节位图，避免切换瞬间闪动。
   */
  async setContent(
    parts: ReaderChapterPart[],
    title: string,
    options: {
      atEnd?: boolean;
      preservePosition?: boolean;
      adopt?: "previous" | "next";
    } = {},
  ): Promise<void> {
    // 在清空前暂存待沿用的边界位图。
    const adopted =
      options.adopt === "next"
        ? this.nextPreview
        : options.adopt === "previous"
          ? this.previousPreview
          : undefined;

    this.parts = parts;
    this.title = title;
    const revision = ++this.revision;
    await this.loadImages(parts);
    if (revision !== this.revision || this.destroyed || !this.style) return;
    this.pruneImages(parts);

    const previous = Math.max(0, Math.round(this.position));
    this.rebuildPages();
    this.bitmaps.clear();
    this.animation = undefined;
    this.previousPreview = undefined;
    this.nextPreview = undefined;
    this.committing = false;

    const count = this.pages.length;
    if (options.adopt === "next" && adopted) {
      this.bitmaps.set(0, adopted);
      this.position = 0;
    } else if (options.adopt === "previous" && adopted) {
      this.bitmaps.set(count - 1, adopted);
      this.position = Math.max(0, count - 1);
    } else if (options.atEnd) {
      this.position = Math.max(0, count - 1);
    } else if (options.preservePosition) {
      this.position = Math.min(previous, Math.max(0, count - 1));
    } else {
      this.position = 0;
    }

    this.resizeCanvas();
    this.render();
    this.emitPage();
  }

  /** 翻到下一页；若已是最后一页则滑向下一章首页或请求进入下一章。 */
  goNext(): void {
    if (this.animation || this.committing || !this.pages.length) return;
    if (this.position >= this.pages.length - 1) {
      if (this.nextPreview) {
        this.animateTo(this.pages.length, FLIP_DURATION);
      } else {
        this.callbacks.onReachEnd?.();
      }
      return;
    }
    this.animateTo(Math.round(this.position) + 1, FLIP_DURATION);
  }

  /** 翻到上一页；若已是第一页则滑向上一章末页或请求返回上一章。 */
  goPrev(): void {
    if (this.animation || this.committing || !this.pages.length) return;
    if (this.position <= 0) {
      if (this.previousPreview) {
        this.animateTo(-1, FLIP_DURATION);
      } else {
        this.callbacks.onReachStart?.();
      }
      return;
    }
    this.animateTo(Math.round(this.position) - 1, FLIP_DURATION);
  }

  /** 释放资源，移除事件监听。 */
  destroy(): void {
    this.destroyed = true;
    this.revision += 1;
    if (this.rafId) {
      cancelAnimationFrame(this.rafId);
      this.rafId = 0;
    }
    this.canvas.removeEventListener("pointerdown", this.handlePointerDown);
    this.canvas.removeEventListener("pointermove", this.handlePointerMove);
    this.canvas.removeEventListener("pointerup", this.handlePointerUp);
    this.canvas.removeEventListener("pointercancel", this.handlePointerCancel);
    this.bitmaps.clear();
    this.images.clear();
    this.pages = [];
  }

  /** 预加载尚未缓存的插图。 */
  private async loadImages(parts: ReaderChapterPart[]): Promise<void> {
    const sources = new Set<string>();
    for (const part of parts) {
      if (
        part.type === "image" &&
        !this.images.has(part.src) &&
        !this.failedImages.has(part.src)
      ) {
        sources.add(part.src);
      }
    }
    if (!sources.size) return;
    await Promise.all(
      [...sources].map(async (src) => {
        try {
          this.images.set(src, await loadImage(src));
        } catch {
          this.failedImages.add(src);
        }
      }),
    );
  }

  /** 丢弃不再被当前章节引用的已加载/失败图片，控制内存。 */
  private pruneImages(parts: ReaderChapterPart[]): void {
    const keep = new Set<string>();
    for (const part of parts) {
      if (part.type === "image") keep.add(part.src);
    }
    for (const src of [...this.images.keys()]) {
      if (!keep.has(src)) this.images.delete(src);
    }
    for (const src of [...this.failedImages]) {
      if (!keep.has(src)) this.failedImages.delete(src);
    }
  }

  /** 依据当前内容与样式重新分页。 */
  private rebuildPages(): void {
    if (!this.style) {
      this.pages = [];
      return;
    }
    this.pages = paginateText({
      parts: this.parts,
      title: this.title,
      style: this.style,
      viewport: this.viewport,
      images: this.images,
    });
  }

  /** 同步 canvas 后备存储尺寸。 */
  private resizeCanvas(): void {
    this.canvas.width = Math.max(
      1,
      Math.round(this.viewport.width * this.dpr),
    );
    this.canvas.height = Math.max(
      1,
      Math.round(this.viewport.height * this.dpr),
    );
  }

  /** 绘制当前帧。 */
  private render(): void {
    const { width, height } = this.viewport;
    if (!width || !height || !this.style) return;
    const ctx = this.ctx;
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    ctx.fillStyle = this.style.background;
    ctx.fillRect(0, 0, width, height);

    if (!this.pages.length) return;
    const base = Math.floor(this.position);
    const fraction = this.position - base;
    this.drawPage(base, -fraction * width);
    this.drawPage(base + 1, (1 - fraction) * width);
  }

  /** 将指定位置（含章节边界预渲染页）绘制到水平偏移处。 */
  private drawPage(index: number, x: number): void {
    const bitmap = this.bitmapAt(index);
    if (bitmap) {
      this.ctx.drawImage(
        bitmap,
        x,
        0,
        this.viewport.width,
        this.viewport.height,
      );
    }
  }

  /** 解析位置对应位图：-1 为上一章末页，count 为下一章首页。 */
  private bitmapAt(index: number): HTMLCanvasElement | undefined {
    if (index === -1) return this.previousPreview;
    if (index === this.pages.length) return this.nextPreview;
    if (index < 0 || index >= this.pages.length) return undefined;
    return this.ensureBitmap(index);
  }

  /** 获取页面位图，未缓存则渲染并做 LRU 淘汰。 */
  private ensureBitmap(index: number): HTMLCanvasElement | undefined {
    const cached = this.bitmaps.get(index);
    if (cached) return cached;
    const page = this.pages[index];
    if (!page || !this.style) return undefined;
    const bitmap = renderPage(page, this.style, this.viewport, this.dpr);
    this.bitmaps.set(index, bitmap);
    this.evictBitmaps(index);
    return bitmap;
  }

  /** 淘汰离当前页最远的位图，控制内存占用。 */
  private evictBitmaps(center: number): void {
    if (this.bitmaps.size <= MAX_BITMAPS) return;
    const keys = [...this.bitmaps.keys()].sort(
      (a, b) => Math.abs(b - center) - Math.abs(a - center),
    );
    while (this.bitmaps.size > MAX_BITMAPS) {
      const farthest = keys.shift();
      if (farthest === undefined) break;
      this.bitmaps.delete(farthest);
    }
  }

  /** 动画到指定页位置（含 -1 与 count 两个章节边界位置）。 */
  private animateTo(target: number, duration: number): void {
    const clamped = this.clampPosition(target);
    if (Math.abs(clamped - this.position) < 0.001) {
      this.position = clamped;
      this.render();
      this.handleSettled(clamped);
      return;
    }
    this.animation = {
      from: this.position,
      to: clamped,
      start: performance.now(),
      duration,
    };
    this.scheduleFrame();
  }

  /** 仅在空闲时请求下一帧。 */
  private scheduleFrame(): void {
    if (!this.rafId) this.rafId = requestAnimationFrame(this.tick);
  }

  /** 动画帧回调。 */
  private tick = (now: number): void => {
    this.rafId = 0;
    const animation = this.animation;
    if (animation) {
      const progress = Math.min(
        1,
        (now - animation.start) / animation.duration,
      );
      this.position =
        animation.from +
        (animation.to - animation.from) * easeOutCubic(progress);
      if (progress >= 1) {
        this.position = animation.to;
        this.animation = undefined;
        this.handleSettled(this.position);
      }
    }
    this.render();
    if (this.animation) this.scheduleFrame();
  };

  /** 动画落定：位于章节边界则请求切换章节，否则上报页码。 */
  private handleSettled(position: number): void {
    if (position === this.pages.length) {
      this.committing = true;
      this.callbacks.onReachEnd?.();
      return;
    }
    if (position === -1) {
      this.committing = true;
      this.callbacks.onReachStart?.();
      return;
    }
    this.emitPage();
  }

  /** 通知外部当前稳定页。 */
  private emitPage(): void {
    const count = this.pages.length;
    if (!count) return;
    const index = Math.min(
      Math.max(0, Math.round(this.position)),
      count - 1,
    );
    this.callbacks.onPageChange?.(index, count);
  }

  /** 越界橡皮筋阻尼：位移越大，增量越小，且不超过上限。 */
  private applyResistance(distance: number): number {
    return (RUBBER_RATIO * distance) / (RUBBER_RATIO + distance);
  }

  /** 把原始位置约束在 [最小位置, 最大位置] 区间，越界部分施加橡皮筋。 */
  private clampPosition(raw: number): number {
    const count = this.pages.length;
    const min = this.previousPreview ? -1 : 0;
    const max = this.nextPreview ? count : Math.max(0, count - 1);
    if (raw < min) return min - this.applyResistance(min - raw);
    if (raw > max) return max + this.applyResistance(raw - max);
    return raw;
  }

  private releasePointer(pointerId: number): void {
    if (this.canvas.hasPointerCapture(pointerId)) {
      this.canvas.releasePointerCapture(pointerId);
    }
  }

  private handlePointerDown = (event: PointerEvent): void => {
    if (this.destroyed || this.committing || this.gesture || event.button > 0)
      return;
    // 拖拽打断正在进行的翻页动画，从当前中间态接管。
    if (this.animation) {
      this.animation = undefined;
      if (this.rafId) {
        cancelAnimationFrame(this.rafId);
        this.rafId = 0;
      }
    }
    this.gesture = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      startPosition: this.position,
      startTime: event.timeStamp,
      lastX: event.clientX,
      lastTime: event.timeStamp,
      velocity: 0,
      axis: null,
    };
    try {
      this.canvas.setPointerCapture(event.pointerId);
    } catch {
      // 指针已释放时忽略。
    }
  };

  private handlePointerMove = (event: PointerEvent): void => {
    const gesture = this.gesture;
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    const dx = event.clientX - gesture.startX;
    const dy = event.clientY - gesture.startY;

    if (gesture.axis === null) {
      if (Math.abs(dx) < AXIS_LOCK && Math.abs(dy) < AXIS_LOCK) return;
      gesture.axis = Math.abs(dx) >= Math.abs(dy) ? "x" : "y";
    }
    if (gesture.axis !== "x" || !this.viewport.width || !this.pages.length)
      return;

    event.preventDefault();
    const deltaTime = event.timeStamp - gesture.lastTime;
    if (deltaTime > 0) {
      const instant =
        -(event.clientX - gesture.lastX) / deltaTime / this.viewport.width;
      // 低通滤波，避免单帧抖动导致误判。
      gesture.velocity = gesture.velocity * 0.6 + instant * 0.4;
      gesture.lastX = event.clientX;
      gesture.lastTime = event.timeStamp;
    }
    this.position = this.clampPosition(
      gesture.startPosition - dx / this.viewport.width,
    );
    this.render();
  };

  private handlePointerUp = (event: PointerEvent): void => {
    const gesture = this.gesture;
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    this.gesture = undefined;
    this.releasePointer(event.pointerId);

    if (gesture.axis !== "x") {
      // 未超过方向阈值且时间足够短，视为点击。
      if (
        gesture.axis === null &&
        event.timeStamp - gesture.startTime <= TAP_TIME
      ) {
        this.emitTap(event);
      }
      return;
    }

    const count = this.pages.length;
    if (!count) return;
    const delta = this.position - gesture.startPosition;
    const idle = event.timeStamp - gesture.lastTime;
    const velocity = idle > 120 ? 0 : gesture.velocity;
    const startIndex = Math.round(gesture.startPosition);
    const wantsNext =
      delta > FLIP_RATIO || (velocity > FLIP_VELOCITY && delta > 0);
    const wantsPrev =
      delta < -FLIP_RATIO || (velocity < -FLIP_VELOCITY && delta < 0);
    const minTarget = this.previousPreview ? -1 : 0;
    const maxTarget = this.nextPreview ? count : count - 1;

    if (startIndex >= count - 1 && wantsNext) {
      if (this.nextPreview) {
        this.animateTo(count, FLIP_DURATION);
      } else {
        this.callbacks.onReachEnd?.();
        this.animateTo(count - 1, FLIP_DURATION);
      }
      return;
    }
    if (startIndex <= 0 && wantsPrev) {
      if (this.previousPreview) {
        this.animateTo(-1, FLIP_DURATION);
      } else {
        this.callbacks.onReachStart?.();
        this.animateTo(0, FLIP_DURATION);
      }
      return;
    }

    let target = Math.round(this.position);
    if (wantsNext) target = Math.min(startIndex + 1, maxTarget);
    else if (wantsPrev) target = Math.max(startIndex - 1, minTarget);
    this.animateTo(target, FLIP_DURATION);
  };

  private handlePointerCancel = (event: PointerEvent): void => {
    if (!this.gesture || this.gesture.pointerId !== event.pointerId) return;
    this.gesture = undefined;
    this.releasePointer(event.pointerId);
    // 取消时回落到当前章节内最近页，不触发跨章切换。
    const count = this.pages.length;
    const target = Math.min(
      Math.max(Math.round(this.position), 0),
      Math.max(0, count - 1),
    );
    this.animateTo(target, 200);
  };

  /** 根据点击位置判定左/中/右分区并上报。 */
  private emitTap(event: PointerEvent): void {
    const rect = this.canvas.getBoundingClientRect();
    const ratio = (event.clientX - rect.left) / Math.max(1, rect.width);
    const zone = ratio < 0.3 ? "prev" : ratio > 0.7 ? "next" : "center";
    this.callbacks.onTap?.(zone);
  }
}
