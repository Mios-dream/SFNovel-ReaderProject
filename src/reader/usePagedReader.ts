/**
 * Canvas 分页阅读器的 Vue 组合式函数。
 *
 * 把与框架无关的 {@link PagedEngine} 接入 Vue：管理 canvas 引用、
 * 尺寸变化监听、样式注入，并在每次布局后预渲染相邻章节的边界页，
 * 以实现跨章节的平滑翻页。
 */

import { onBeforeUnmount, ref, watch, type Ref } from "vue";
import { PagedEngine } from "./pagedEngine";
import type { ReaderChapterPart, ReaderStyle } from "./types";

/** 相邻章节的正文数据，用于预渲染边界页。 */
export interface NeighborChapterContent {
  parts: ReaderChapterPart[];
  title: string;
}

/** 布局参数。 */
export interface PagedLayoutOptions {
  /** 定位到最后一页（用于上一章回退）。 */
  atEnd?: boolean;
  /** 尽量保持当前页索引（用于尺寸变化）。 */
  preservePosition?: boolean;
  /** 复用边界滑动时已显示的相邻章节位图，避免切换瞬间闪动。 */
  adopt?: "previous" | "next";
}

/** 组合式函数配置。 */
export interface UsePagedReaderOptions {
  /** 承载分页画面的 canvas 元素引用。 */
  canvas: Ref<HTMLCanvasElement | undefined>;
  /** 返回当前章节的正文片段。 */
  parts: () => ReaderChapterPart[];
  /** 返回当前章节标题。 */
  title: () => string;
  /** 返回当前排版样式。 */
  style: () => ReaderStyle;
  /** 读取相邻章节内容（不改变阅读状态），用于预渲染边界页。 */
  prepareNeighbor?: (
    direction: "previous" | "next",
  ) => Promise<NeighborChapterContent | undefined>;
  /** 在最后一页继续前进时回调（用于加载下一章）。 */
  onReachEnd?: () => void;
  /** 在第一页继续后退时回调（用于加载上一章）。 */
  onReachStart?: () => void;
  /** 点击左/中/右分区时回调。 */
  onTap?: (zone: "prev" | "center" | "next") => void;
}

/**
 * 创建分页阅读器控制器。
 * @returns 响应式页码、总页数，以及重新布局/翻页方法。
 */
export function usePagedReader(options: UsePagedReaderOptions) {
  const pageIndex = ref(0);
  const pageCount = ref(0);
  let engine: PagedEngine | undefined;
  let resizeObserver: ResizeObserver | undefined;
  let resizeTimer: number | undefined;
  let lastWidth = 0;
  let lastHeight = 0;
  /** 预渲染版本号，用于丢弃过期的相邻章节位图。 */
  let previewToken = 0;

  /** 预渲染相邻章节的边界页并交给引擎。 */
  async function preparePreviews(): Promise<void> {
    const instance = engine;
    if (!instance || !options.prepareNeighbor) return;
    const token = previewToken;
    const [previous, next] = await Promise.all([
      options.prepareNeighbor("previous"),
      options.prepareNeighbor("next"),
    ]);
    if (token !== previewToken || instance !== engine) return;
    const [previousBitmap, nextBitmap] = await Promise.all([
      previous
        ? instance.buildPageBitmap(previous.parts, previous.title, "last")
        : undefined,
      next
        ? instance.buildPageBitmap(next.parts, next.title, "first")
        : undefined,
    ]);
    if (token !== previewToken || instance !== engine) return;
    instance.setPreviews({ previous: previousBitmap, next: nextBitmap });
  }

  /** 依据 canvas 实际尺寸执行分页布局。 */
  async function applyLayout(opts: PagedLayoutOptions = {}): Promise<void> {
    const canvas = options.canvas.value;
    if (!canvas || !canvas.parentElement) return;
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    // 尺寸过小时跳过，等待 ResizeObserver 在可用后再布局。
    if (width < 120 || height < 160) return;
    lastWidth = width;
    lastHeight = height;
    const layoutToken = ++previewToken;

    if (!engine) {
      engine = new PagedEngine(canvas, {
        onPageChange: (index, total) => {
          pageIndex.value = index;
          pageCount.value = total;
        },
        onReachEnd: () => options.onReachEnd?.(),
        onReachStart: () => options.onReachStart?.(),
        onTap: (zone) => options.onTap?.(zone),
      });
    }
    engine.configure(options.style(), { width, height });
    await engine.setContent(options.parts(), options.title(), {
      atEnd: opts.atEnd,
      preservePosition: opts.preservePosition,
      adopt: opts.adopt,
    });
    // 已被更新的布局取代时不再预渲染，避免写入过期位图。
    if (layoutToken !== previewToken) return;
    void preparePreviews();
  }

  /** 尺寸变化时防抖重排，并过滤地址栏收放带来的小幅高度抖动。 */
  function scheduleResize(): void {
    window.clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(() => {
      const canvas = options.canvas.value;
      if (!canvas) return;
      const width = canvas.clientWidth;
      const height = canvas.clientHeight;
      if (width === lastWidth && Math.abs(height - lastHeight) < 120) return;
      void applyLayout({ preservePosition: true });
    }, 160);
  }

  /** 重新挂载尺寸监听。 */
  function attachObserver(): void {
    resizeObserver?.disconnect();
    const target = options.canvas.value?.parentElement;
    if (!target) return;
    resizeObserver = new ResizeObserver(scheduleResize);
    resizeObserver.observe(target);
  }

  /** 释放引擎与监听。 */
  function dispose(): void {
    window.clearTimeout(resizeTimer);
    resizeObserver?.disconnect();
    resizeObserver = undefined;
    previewToken += 1;
    engine?.destroy();
    engine = undefined;
  }

  watch(
    options.canvas,
    (canvas) => {
      if (canvas) attachObserver();
      else dispose();
    },
    { flush: "post" },
  );

  onBeforeUnmount(dispose);

  return {
    pageIndex,
    pageCount,
    /** 重新分页布局。 */
    relayout: (opts?: PagedLayoutOptions) => applyLayout(opts),
    goNext: () => engine?.goNext(),
    goPrev: () => engine?.goPrev(),
  };
}
