<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import {
  ChevronLeft,
  ChevronRight,
  Columns2,
  Images,
  ListTree,
  LoaderCircle,
  MapPin,
  Maximize,
  ScrollText,
  ZoomIn,
  ZoomOut,
} from "@lucide/vue";
import { useRouter } from "vue-router";
import DrawerShell from "../components/DrawerShell.vue";
import DirectoryGroupTitle, {
  DIRECTORY_GROUP_HEIGHT,
} from "../components/DirectoryGroupTitle.vue";
import ReaderChrome, {
  type ReaderAction,
} from "../components/ReaderChrome.vue";
import { useVirtualRows } from "../composables/useVirtualRows";
import type { DisplayMode } from "../types";

import { useAuth, useLibrary, useOnline, useWorkspace } from "../desk";

const auth = useAuth();
const library = useLibrary();
const reading = useOnline();
const workspace = useWorkspace();


const router = useRouter();
const readerElement = ref<HTMLElement>();
const pageViewport = ref<HTMLElement>();
const book = computed(() => library.localBook.value);
const online = computed(() => workspace.readerSource.value === "online");
const comicChapter = computed(() =>
  online.value ? reading.comicChapter.value : library.localComicChapter.value,
);
const bookName = computed(() =>
  online.value
    ? reading.onlineBook.value?.novelName || "在线漫画"
    : book.value?.name || "本地漫画",
);
type ComicChapterEntry = {
  id: number;
  title: string;
  cover?: string;
  isVip?: boolean;
};
const chapters = computed<ComicChapterEntry[]>(() => {
  if (online.value)
    return reading.comicChapters.value.map((chapter) => ({
      id: chapter.id,
      title: chapter.title,
      isVip: chapter.isVip,
      // 在线目录的预览图来自上游 `expand.chapCover`，本地章节取首图兜底。
      cover: chapter.cover,
    }));
  return (book.value?.comicChapters || []).map((chapter) => ({
    id: chapter.id,
    title: chapter.title,
    cover: chapter.cover,
  }));
});
const directoryRows = computed(() => [
  { kind: "header" as const, key: "header", height: DIRECTORY_GROUP_HEIGHT },
  ...chapters.value.map((item, index) => ({
    kind: "chapter" as const,
    key: `chapter-${item.id}`,
    index,
    item,
    height: 68,
  })),
]);
const {
  container: directoryList,
  windowed: directoryWindow,
  handleScroll: handleDirectoryScroll,
} = useVirtualRows(() => directoryRows.value);
const chapterIndex = computed(() =>
  chapters.value.findIndex((item) => item.id === comicChapter.value?.id),
);
const previousChapter = computed(() => chapters.value[chapterIndex.value - 1]);
const nextChapter = computed(() => chapters.value[chapterIndex.value + 1]);
const controlsVisible = ref(true);
const directoryOpen = ref(false);
const displayMode = ref<DisplayMode>("vertical");
type ComicPanel = "zoom" | "mode";
/** null 表示坞里显示翻页进度条。 */
const activePanel = ref<ComicPanel | null>(null);
const pageIndex = ref(0);
/** 竖屏模式下的滚动进度，与翻页模式的页码进度共用同一条进度条。 */
const scrollProgress = ref(0);
const chapterLoading = ref(false);
const pageCount = computed(() => comicChapter.value?.pages.length || 0);
const progressPercent = computed(() => {
  if (displayMode.value === "paged")
    return pageCount.value
      ? Math.round(((pageIndex.value + 1) / pageCount.value) * 100)
      : 0;
  return Math.round(scrollProgress.value);
});
const canGoPrevious = computed(
  () =>
    Boolean(previousChapter.value) ||
    (displayMode.value === "paged" && pageIndex.value > 0),
);
const canGoNext = computed(
  () =>
    Boolean(nextChapter.value) ||
    (displayMode.value === "paged" && pageIndex.value < pageCount.value - 1),
);
const requestedChapterId = ref<number>();
const onlineError = computed(() =>
  online.value ? reading.chapterError.value || reading.directoryError.value : "",
);
const onlineLoading = computed(
  () =>
    online.value && (reading.chapterLoading.value || reading.directoryLoading.value),
);
const onlineNeedsAuth = computed(
  () => online.value && !auth.auth.value.webAuthenticated,
);
const firstPageReady = ref(false);
const firstPageError = ref("");
const comicFirstPageLoading = computed(
  () =>
    online.value &&
    Boolean(comicChapter.value) &&
    !firstPageReady.value &&
    !firstPageError.value,
);
const comicStateTitle = computed(() => {
  if (
    chapterLoading.value ||
    onlineLoading.value ||
    comicFirstPageLoading.value
  )
    return "正在加载漫画页面";
  if (onlineError.value || firstPageError.value) return "加载失败";
  return "从目录选择一章开始阅读";
});
const carouselOffset = ref(0);
const carouselTransitioning = ref(false);
const imageScale = ref(1);
const imageOffset = ref({ x: 0, y: 0 });
let flipPointerStart: { x: number; y: number } | undefined;
let ignoreNextReaderClick = false;
const activePointers = new Map<number, { x: number; y: number }>();
let pinchStartDistance = 0;
let pinchStartScale = 1;
let panStart:
  | { x: number; y: number; offsetX: number; offsetY: number }
  | undefined;
let firstPageProbe: HTMLImageElement | undefined;

const carouselPages = computed(() => {
  const pages = comicChapter.value?.pages || [];
  return [pageIndex.value - 1, pageIndex.value, pageIndex.value + 1].map(
    (index) => ({ index, src: pages[index] }),
  );
});

const imageTransform = computed(
  () =>
    `translate3d(${imageOffset.value.x}px, ${imageOffset.value.y}px, 0) scale(${imageScale.value})`,
);

function resetImageView() {
  imageScale.value = 1;
  imageOffset.value = { x: 0, y: 0 };
}

function resetCarouselPosition() {
  carouselOffset.value = 0;
  carouselTransitioning.value = false;
}

function preloadComicFirstPage(src?: string) {
  firstPageProbe?.removeAttribute("src");
  firstPageProbe = undefined;
  firstPageReady.value = false;
  firstPageError.value = "";
  if (!src) {
    firstPageError.value = "漫画首图地址无效";
    return;
  }
  const probe = new Image();
  firstPageProbe = probe;
  probe.decoding = "async";
  probe.onload = () => {
    if (firstPageProbe !== probe) return;
    firstPageReady.value = true;
  };
  probe.onerror = () => {
    if (firstPageProbe !== probe) return;
    firstPageError.value = "漫画首图加载失败，请重试";
  };
  probe.src = src;
}

function setImageScale(nextScale: number) {
  imageScale.value = Math.min(3, Math.max(1, nextScale));
  if (imageScale.value === 1) imageOffset.value = { x: 0, y: 0 };
}

function zoomImage(delta: number) {
  setImageScale(imageScale.value + delta);
}

function animateCarousel(direction: "forward" | "back") {
  if (carouselTransitioning.value) return;
  const pages = comicChapter.value?.pages || [];
  const target = pageIndex.value + (direction === "forward" ? 1 : -1);
  if (target < 0 || target >= pages.length) {
    carouselTransitioning.value = true;
    carouselOffset.value = 0;
    window.setTimeout(resetCarouselPosition, 220);
    return;
  }
  carouselTransitioning.value = true;
  const width = pageViewport.value?.clientWidth || window.innerWidth;
  carouselOffset.value = direction === "forward" ? -width : width;
  window.setTimeout(() => {
    pageIndex.value = target;
    resetCarouselPosition();
    resetImageView();
  }, 240);
}

function distanceBetweenPointers() {
  const points = [...activePointers.values()];
  if (points.length < 2) return 0;
  return Math.hypot(points[0].x - points[1].x, points[0].y - points[1].y);
}

async function openChapter(chapterId?: number, showLastPage = false) {
  if (!chapterId || chapterLoading.value) return;
  chapterLoading.value = true;
  requestedChapterId.value = chapterId;
  try {
    if (online.value) await reading.openOnlineComicChapter(chapterId);
    else await library.openLocalComicChapter(chapterId);
    if (comicChapter.value?.id !== chapterId) return;
    pageIndex.value = showLastPage
      ? Math.max(0, (comicChapter.value.pages.length || 1) - 1)
      : 0;
    resetImageView();
    resetCarouselPosition();
    scrollProgress.value = 0;
    directoryOpen.value = false;
    await nextTick();
    readerElement.value?.scrollTo({ top: 0 });
  } finally {
    chapterLoading.value = false;
  }
}

function goBack() {
  workspace.back();
}

async function retryOnlineLoad() {
  try {
    await reading.loadDirectory(true);
    const targetId =
      requestedChapterId.value ||
      reading.pendingChapterId.value ||
      chapters.value[0]?.id;
    if (targetId) {
      await openChapter(targetId);
      if (comicChapter.value?.id === targetId)
        preloadComicFirstPage(comicChapter.value.pages[0]);
    }
  } catch {
    // 目录错误会继续展示在页面中。
  }
}

function previous() {
  if (displayMode.value === "paged" && pageIndex.value > 0) {
    animateCarousel("back");
    return;
  }
  void openChapter(previousChapter.value?.id, displayMode.value === "paged");
}

function next() {
  if (displayMode.value === "paged" && pageIndex.value < pageCount.value - 1) {
    animateCarousel("forward");
    return;
  }
  void openChapter(nextChapter.value?.id);
}

function handleReaderScroll() {
  const element = readerElement.value;
  if (!element || displayMode.value !== "vertical") return;
  const scrollable = element.scrollHeight - element.clientHeight;
  scrollProgress.value =
    scrollable > 0 ? Math.min(100, (element.scrollTop / scrollable) * 100) : 0;
  if (element.scrollTop + element.clientHeight >= element.scrollHeight - 32)
    void openChapter(nextChapter.value?.id);
}

function setDisplayMode(mode: DisplayMode) {
  if (displayMode.value === mode) return;
  displayMode.value = mode;
  resetImageView();
  scrollProgress.value = 0;
  void nextTick(() => {
    readerElement.value?.scrollTo({ top: 0 });
  });
}

function toggleControls() {
  controlsVisible.value = !controlsVisible.value;
  if (!controlsVisible.value) activePanel.value = null;
}

function handleReaderClick(event: MouseEvent) {
  if (ignoreNextReaderClick) {
    ignoreNextReaderClick = false;
    return;
  }
  const rect = readerElement.value?.getBoundingClientRect();
  if (!rect) return;
  if (displayMode.value === "paged") {
    // 与小说翻页一致：左右三分区翻页，中间切换菜单（阈值同 pagedEngine）。
    const ratio = (event.clientX - rect.left) / Math.max(1, rect.width);
    if (ratio < 0.3) previous();
    else if (ratio > 0.7) next();
    else toggleControls();
    return;
  }
  const localY = event.clientY - rect.top;
  if (localY < rect.height * 0.25 || localY > rect.height * 0.75) return;
  toggleControls();
}

function handleFlipPointerDown(event: PointerEvent) {
  if (displayMode.value !== "paged" || carouselTransitioning.value) return;
  // 从按钮上开始的按下不接管翻页手势：一旦在舞台容器上 setPointerCapture，
  // pointerup 会被改派到这里，顺带把 click 改派成容器，按钮自己的 click 收不到。
  if (
    event.target instanceof Element &&
    event.target.closest("button, a, [role='button']")
  )
    return;
  activePointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
  event.currentTarget instanceof HTMLElement &&
    event.currentTarget.setPointerCapture(event.pointerId);
  if (activePointers.size >= 2) {
    pinchStartDistance = distanceBetweenPointers();
    pinchStartScale = imageScale.value;
    panStart = undefined;
    return;
  }
  flipPointerStart = { x: event.clientX, y: event.clientY };
  carouselOffset.value = 0;
  if (imageScale.value > 1) {
    panStart = {
      x: event.clientX,
      y: event.clientY,
      offsetX: imageOffset.value.x,
      offsetY: imageOffset.value.y,
    };
  }
}

function handleFlipPointerMove(event: PointerEvent) {
  if (displayMode.value !== "paged" || !activePointers.has(event.pointerId))
    return;
  activePointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
  if (activePointers.size >= 2) {
    const distance = distanceBetweenPointers();
    if (pinchStartDistance > 0)
      setImageScale(pinchStartScale * (distance / pinchStartDistance));
    return;
  }
  if (panStart && imageScale.value > 1) {
    imageOffset.value = {
      x: panStart.offsetX + event.clientX - panStart.x,
      y: panStart.offsetY + event.clientY - panStart.y,
    };
  } else if (imageScale.value === 1 && activePointers.size === 1) {
    carouselOffset.value =
      event.clientX - (flipPointerStart?.x || event.clientX);
  }
}

function handleFlipPointerUp(event: PointerEvent) {
  activePointers.delete(event.pointerId);
  if (activePointers.size) return;
  const start = flipPointerStart;
  flipPointerStart = undefined;
  const wasPanning = Boolean(panStart);
  panStart = undefined;
  pinchStartDistance = 0;
  if (
    !start ||
    displayMode.value !== "paged" ||
    imageScale.value > 1 ||
    wasPanning
  )
    return;
  const deltaX = event.clientX - start.x;
  const deltaY = Math.abs(event.clientY - start.y);
  if (Math.abs(deltaX) < 36 || Math.abs(deltaX) < deltaY * 1.2) {
    carouselTransitioning.value = true;
    window.setTimeout(resetCarouselPosition, 180);
    return;
  }
  ignoreNextReaderClick = true;
  window.setTimeout(() => {
    ignoreNextReaderClick = false;
  }, 350);
  if (deltaX < 0) next();
  else previous();
}

function handleFlipPointerCancel(event: PointerEvent) {
  activePointers.delete(event.pointerId);
  flipPointerStart = undefined;
  panStart = undefined;
  pinchStartDistance = 0;
  resetCarouselPosition();
}

/** 底部控制栏的三个入口；顺序与小说阅读器一致。 */
const comicActions = computed<ReaderAction[]>(() => [
  { key: "directory", label: "目录" },
  { key: "zoom", label: "缩放", active: activePanel.value === "zoom" },
  { key: "mode", label: "阅读方式", active: activePanel.value === "mode" },
]);

function toggleComicPanel(panel: ComicPanel) {
  activePanel.value = activePanel.value === panel ? null : panel;
}

function handleComicAction(key: string) {
  if (key === "directory") showDirectory();
  else if (key === "zoom" || key === "mode") toggleComicPanel(key);
}

function showDirectory() {
  directoryOpen.value = true;
  activePanel.value = null;
}

function setDirectoryBody(element: HTMLElement | null) {
  directoryList.value = element ?? undefined;
}

async function revealCurrentChapter() {
  if (!directoryOpen.value || reading.directoryLoading.value) return;
  await nextTick();
  const list = directoryList.value;
  if (!list) return;
  if (chapterIndex.value >= 0)
    list.scrollTop = Math.max(
      0,
      44 +
        chapterIndex.value * 68 -
        (list.clientHeight - 68) / 2,
    );
  handleDirectoryScroll();
}

onMounted(async () => {
  if (online.value) {
    if (!reading.onlineBook.value) {
      void router.replace("/remote-book");
      return;
    }
    try {
      await reading.loadDirectory();
      const targetId =
        reading.comicChapter.value?.id ||
        reading.pendingChapterId.value ||
        chapters.value[0]?.id;
      if (targetId && reading.comicChapter.value?.id !== targetId)
        await openChapter(targetId);
    } catch {
      // 错误已保存在共享状态中并由页面展示。
    }
    return;
  }
  if (!book.value || !comicChapter.value) {
    void router.replace("/library");
    return;
  }
  workspace.navigate("comicReader");
});

watch(
  () => reading.pendingChapterId.value,
  (chapterId) => {
    if (
      online.value &&
      chapterId &&
      reading.comicChapter.value?.id !== chapterId
    ) {
      void openChapter(chapterId);
    }
  },
);

watch(
  [directoryOpen, () => reading.directoryLoading.value, () => comicChapter.value?.id],
  () => void revealCurrentChapter(),
);

watch(
  () => comicChapter.value,
  (chapter) => {
    if (!online.value) {
      firstPageReady.value = Boolean(chapter);
      firstPageError.value = "";
      return;
    }
    if (!chapter) {
      firstPageProbe?.removeAttribute("src");
      firstPageProbe = undefined;
      firstPageReady.value = false;
      firstPageError.value = "";
      return;
    }
    preloadComicFirstPage(chapter.pages[0]);
  },
  { immediate: true },
);
</script>

<template>
  <article
    v-if="comicChapter && (!online || firstPageReady)"
    ref="readerElement"
    class="comic-reader"
    @scroll="handleReaderScroll"
    @click="handleReaderClick"
  >
    <div v-if="onlineError" class="comic-error-banner" @click.stop>
      <span>{{ onlineError }}</span>
      <button
        v-if="onlineNeedsAuth"
        type="button"
        @click="auth.openCredentials('official')"
      >
        连接网站登录
      </button>
      <button type="button" @click="retryOnlineLoad">重试</button>
    </div>

    <main class="comic-stage" :class="{ paged: displayMode === 'paged' }">
      <template v-if="displayMode === 'vertical'">
        <img
          v-for="(page, index) in comicChapter.pages"
          :key="page"
          class="comic-page-image"
          :src="page"
          :alt="`第 ${index + 1} 页`"
          :loading="index === 0 ? 'eager' : 'lazy'"
          decoding="async"
          :fetchpriority="index === 0 ? 'high' : 'auto'"
        />
      </template>
      <div
        v-else
        class="comic-single-page"
        @pointerdown="handleFlipPointerDown"
        @pointermove="handleFlipPointerMove"
        @pointerup="handleFlipPointerUp"
        @pointercancel="handleFlipPointerCancel"
      >
        <header
          v-show="!controlsVisible"
          class="comic-page-chrome comic-page-topbar"
          @click.stop
        >
          <button
            class="comic-page-back"
            title="返回书籍详情"
            aria-label="返回书籍详情"
            @click="goBack"
          >
            <ChevronLeft :size="26" />
          </button>
          <strong>{{ comicChapter.title }}</strong>
        </header>
        <div
          ref="pageViewport"
          class="comic-image-viewport"
          @wheel.prevent="
            setImageScale(imageScale + ($event.deltaY < 0 ? 0.15 : -0.15))
          "
        >
          <div
            class="comic-carousel-track"
            :class="{ transitioning: carouselTransitioning }"
            :style="{
              transform: `translate3d(calc(-33.333333% + ${carouselOffset}px), 0, 0)`,
            }"
          >
            <div
              v-for="page in carouselPages"
              :key="page.index"
              class="comic-carousel-slide"
            >
              <img
                v-if="page.src"
                class="comic-paged-image"
                :src="page.src"
                :alt="`第 ${page.index + 1} 页`"
                :loading="page.index === pageIndex ? 'eager' : 'lazy'"
                decoding="async"
                :style="{ transform: imageTransform }"
                @dblclick.stop="
                  imageScale > 1 ? resetImageView() : setImageScale(2)
                "
              />
            </div>
          </div>
        </div>
        <div
          v-show="!controlsVisible"
          class="comic-page-chrome comic-page-bottom-meta"
          aria-label="书名和页数"
        >
          <span>{{ bookName }}</span>
          <span>{{ pageIndex + 1 }} / {{ pageCount }}</span>
        </div>
      </div>
    </main>

    <ReaderChrome
      :visible="controlsVisible"
      :subtitle="`第 ${chapterIndex + 1} 章`"
      :title="comicChapter.title"
      :actions="comicActions"
      @back="goBack"
      @action="handleComicAction"
    >
      <template #panel>
        <Transition name="reader-swap" mode="out-in">
          <section
            v-if="activePanel === 'zoom'"
            key="zoom"
            class="reader-panel"
            aria-label="画面缩放"
          >
            <div class="reader-setting-row">
              <span class="reader-setting-label">缩放</span>
              <div class="reader-stepper">
                <button
                  type="button"
                  title="缩小"
                  aria-label="缩小"
                  :disabled="imageScale <= 1"
                  @click="zoomImage(-0.25)"
                >
                  <ZoomOut :size="16" />
                </button>
                <span class="reader-stepper-value"
                  >{{ Math.round(imageScale * 100) }}%</span
                >
                <button
                  type="button"
                  title="放大"
                  aria-label="放大"
                  :disabled="imageScale >= 3"
                  @click="zoomImage(0.25)"
                >
                  <ZoomIn :size="16" />
                </button>
                <span class="reader-stepper-divider" aria-hidden="true"></span>
                <button
                  type="button"
                  title="适应屏幕"
                  aria-label="适应屏幕"
                  :disabled="imageScale === 1"
                  @click="resetImageView()"
                >
                  <Maximize :size="15" />
                </button>
              </div>
            </div>
          </section>
          <section
            v-else-if="activePanel === 'mode'"
            key="mode"
            class="reader-panel"
            aria-label="阅读方式设置"
          >
            <div class="reader-mode-options">
              <button
                type="button"
                :class="{ active: displayMode === 'vertical' }"
                :aria-pressed="displayMode === 'vertical'"
                @click="setDisplayMode('vertical')"
              >
                <ScrollText :size="18" /><span>滚动</span>
              </button>
              <button
                type="button"
                :class="{ active: displayMode === 'paged' }"
                :aria-pressed="displayMode === 'paged'"
                @click="setDisplayMode('paged')"
              >
                <Columns2 :size="18" /><span>翻页</span>
              </button>
            </div>
          </section>
          <div v-else key="progress" class="reader-progress-control">
            <button
              type="button"
              class="reader-turn-button"
              title="上一章或上一页"
              aria-label="上一章或上一页"
              :disabled="!canGoPrevious"
              @click="previous"
            >
              <ChevronLeft :size="20" />
            </button>
            <div class="reader-progress-main">
              <div
                class="reader-progress-track"
                role="progressbar"
                aria-label="本章阅读进度"
                aria-valuemin="0"
                aria-valuemax="100"
                :aria-valuenow="progressPercent"
              >
                <span :style="{ width: `${progressPercent}%` }"></span>
              </div>
            </div>
            <button
              type="button"
              class="reader-turn-button"
              title="下一章或下一页"
              aria-label="下一章或下一页"
              :disabled="!canGoNext"
              @click="next"
            >
              <ChevronRight :size="20" />
            </button>
          </div>
        </Transition>
      </template>

      <template #directory><ListTree :size="20" /></template>
      <template #zoom><ZoomIn :size="20" /></template>
      <template #mode>
        <ScrollText v-if="displayMode === 'vertical'" :size="20" />
        <Columns2 v-else :size="20" />
      </template>
    </ReaderChrome>
  </article>

  <section v-else class="comic-reader-state">
    <button
      class="comic-state-back"
      type="button"
      title="返回书籍详情"
      aria-label="返回书籍详情"
      @click="goBack"
    >
      <ChevronLeft :size="19" /><span>返回</span>
    </button>
    <LoaderCircle
      v-if="chapterLoading || onlineLoading || comicFirstPageLoading"
      class="spin"
      :size="31"
    />
    <Images v-else :size="32" />
    <strong>{{ comicStateTitle }}</strong>
    <span v-if="onlineError">{{ onlineError }}</span>
    <span v-else-if="firstPageError">{{ firstPageError }}</span>
    <span v-else-if="chapterLoading || onlineLoading || comicFirstPageLoading"
      >首图加载完成后立即显示，后续页面会随阅读按需加载</span
    >
    <div class="comic-state-actions">
      <button
        v-if="onlineNeedsAuth"
        type="button"
        @click="auth.openCredentials('official')"
      >
        连接网站登录
      </button>
      <button
        v-if="
          online && !onlineLoading && !chapterLoading && !comicFirstPageLoading
        "
        type="button"
        @click="retryOnlineLoad"
      >
        重试
      </button>
    </div>
  </section>

  <DrawerShell
    :open="directoryOpen"
    align="start"
    title="章节目录"
    :subtitle="bookName"
    :body-ref="setDirectoryBody"
    @close="directoryOpen = false"
    @scroll="handleDirectoryScroll"
  >
    <template #actions>
      <span v-if="chapters.length" class="directory-count"
        >{{ chapters.length }} 章</span
      >
    </template>
    <div v-if="reading.directoryLoading.value" class="drawer-state">
      <LoaderCircle class="spin" :size="24" />正在读取目录
    </div>
    <div v-else-if="reading.directoryError.value" class="drawer-state error">
      <span>{{ reading.directoryError.value }}</span>
      <button type="button" @click="retryOnlineLoad">重试</button>
    </div>
    <template v-else-if="chapters.length">
      <div
        :style="{ height: `${directoryWindow.top}px` }"
        aria-hidden="true"
      ></div>
      <template v-for="row in directoryWindow.rows" :key="row.key">
        <DirectoryGroupTitle v-if="row.kind === 'header'" label="全部章节" />
        <button
          v-else
          class="directory-item directory-chapter"
          :class="{ active: row.item.id === comicChapter?.id }"
          @click="openChapter(row.item.id)"
        >
          <img
            v-if="row.item.cover"
            class="directory-cover"
            :src="row.item.cover"
            :alt="`${row.item.title} 首图`"
          />
          <strong>{{ row.item.title }}</strong>
          <span class="directory-row-meta">
            <small v-if="row.item.isVip" class="vip-badge">VIP</small>
            <MapPin
              v-if="row.item.id === comicChapter?.id"
              class="directory-current-icon"
              :size="16"
              aria-hidden="true"
            />
          </span>
        </button>
      </template>
      <div
        :style="{ height: `${directoryWindow.bottom}px` }"
        aria-hidden="true"
      ></div>
    </template>
    <div v-else class="drawer-state">暂无目录内容</div>
  </DrawerShell>
</template>

<style scoped>
.comic-reader {
  position: fixed;
  inset: 0;
  overflow-y: auto;
  background: var(--background-color);
  color: var(--ink);
}
.comic-stage {
  display: flex;
  width: min(900px, 100%);
  min-height: 100%;
  flex-direction: column;
  align-items: center;
  margin: 0 auto;
}
.comic-page-image {
  display: block;
  width: min(100%, 760px);
  height: auto;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
  cursor: default;
  pointer-events: none;
}
.comic-stage.paged {
  box-sizing: border-box;
  height: 100dvh;
  min-height: 100dvh;
  justify-content: center;
}
.comic-single-page {
  position: relative;
  display: flex;
  width: 100%;
  height: 100%;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  overflow: hidden;
  touch-action: none;
}
.comic-image-viewport {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
}
.comic-carousel-track {
  display: flex;
  width: 300%;
  height: 100%;
  transform: translate3d(-33.333333%, 0, 0);
  will-change: transform;
}
.comic-carousel-track.transitioning {
  transition: transform 240ms cubic-bezier(0.22, 0.61, 0.36, 1);
}
.comic-carousel-slide {
  display: flex;
  width: 33.333333%;
  height: 100%;
  flex: 0 0 33.333333%;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}
.comic-page-chrome {
  position: absolute;
  z-index: 20;
  color: var(--theme-color-dark);
  /* 仅按钮本身开启命中，容器不遮挡翻页手势。 */
  pointer-events: none;
}
.comic-page-topbar {
  top: max(12px, env(safe-area-inset-top));
  right: 0;
  left: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 42px;
  padding: 0 54px;
}
.comic-page-topbar strong {
  max-width: min(72%, 680px);
  overflow: hidden;
  font-size: 14px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.comic-page-back {
  position: absolute;
  left: max(10px, env(safe-area-inset-left));
  display: inline-flex;
  width: 42px;
  height: 42px;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 50%;
  color: var(--theme-color-dark);
  background: transparent;
  pointer-events: auto;
}
.comic-page-back:hover {
  background: var(--theme-color-light);
}
.comic-page-bottom-meta {
  right: max(14px, env(safe-area-inset-right));
  bottom: max(14px, env(safe-area-inset-bottom));
  display: flex;
  max-width: min(60%, 360px);
  gap: 8px;
  align-items: center;
  color: var(--muted);
  font-size: 11px;
  line-height: 1.2;
}
.comic-page-bottom-meta span:first-child {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.comic-single-page img {
  display: block;
  max-width: 100%;
  max-height: 100dvh;
  object-fit: contain;
  cursor: default;
  pointer-events: none;
}
.comic-paged-image {
  width: auto;
  max-width: 100%;
  pointer-events: none;
  user-select: none;
  touch-action: none;
  will-change: transform;
}

.directory-count {
  padding: 5px 9px;
  border-radius: 999px;
  color: #b8664e;
  background: #fff0e7;
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
}
.directory-item {
  display: flex;
  width: 100%;
  height: 68px;
  min-height: 68px;
  align-items: center;
  gap: 12px;
  padding: 9px 20px;
  border: 0;
  border-top: 1px solid #f0ebdf;
  color: #504a41;
  background: transparent;
  font: inherit;
  font-size: 13px;
  font-weight: 500;
  text-align: left;
  transition:
    color 0.16s ease,
    background 0.16s ease;
}
.directory-item:hover {
  color: #bd6548;
  background: #fff9f1;
}
.directory-item.active {
  color: #b85e42;
  background: #fff4ea;
  box-shadow: inset 3px 0 0 #e67c55;
  font-weight: 600;
}
.directory-cover {
  width: 64px;
  height: 46px;
  flex: 0 0 auto;
  border: 1px solid #eaded1;
  border-radius: 6px;
  object-fit: cover;
}
.directory-item strong {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  font-weight: inherit;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.directory-row-meta {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 7px;
  margin-left: auto;
}
.directory-current-icon {
  flex: 0 0 auto;
  color: #d87a56;
}
.vip-badge {
  display: inline-flex;
  min-height: 18px;
  flex: 0 0 auto;
  align-items: center;
  padding: 0 5px;
  border-radius: 4px;
  color: #a45b3f;
  background: #fbe2d4;
  font-size: 9px;
  white-space: nowrap;
}
.comic-error-banner {
  position: fixed;
  z-index: 31;
  top: calc(max(12px, env(safe-area-inset-top)) + 62px);
  right: 14px;
  left: 14px;
  display: flex;
  align-items: center;
  gap: 8px;
  max-width: 760px;
  margin: 0 auto;
  padding: 9px 12px;
  border: 1px solid rgba(224, 176, 154, 0.7);
  border-radius: 8px;
  color: #8f4b36;
  background: rgba(255, 247, 241, 0.96);
  box-shadow: 0 8px 24px rgba(137, 76, 55, 0.12);
  font-size: 12px;
}
.comic-error-banner span {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.comic-error-banner button,
.comic-state-actions button {
  flex: 0 0 auto;
  padding: 6px 10px;
  border: 1px solid rgba(214, 143, 113, 0.56);
  border-radius: 6px;
  color: var(--theme-color-dark);
  background: #fffaf7;
  font-size: 12px;
}
.comic-error-banner button:hover,
.comic-state-actions button:hover {
  color: #fff;
  background: var(--theme-color);
}
.comic-reader-state {
  position: fixed;
  inset: 0;
  display: grid;
  align-content: center;
  justify-items: center;
  gap: 10px;
  padding: 32px;
  color: var(--theme-color-dark);
  background: var(--background-color);
  text-align: center;
}
.comic-state-back {
  position: fixed;
  z-index: 32;
  top: max(12px, env(safe-area-inset-top));
  left: 14px;
  display: inline-flex;
  min-height: 40px;
  align-items: center;
  gap: 4px;
  padding: 0 11px 0 7px;
  border: 1px solid rgba(224, 176, 154, 0.7);
  border-radius: 999px;
  color: var(--theme-color-dark);
  background: rgba(255, 250, 247, 0.94);
  box-shadow: 0 7px 20px rgba(137, 76, 55, 0.12);
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}
.comic-state-back:hover {
  color: #fff;
  background: var(--theme-color);
}
.comic-reader-state strong {
  font-size: 16px;
}
.comic-reader-state > span {
  max-width: min(560px, 86vw);
  color: var(--muted);
  font-size: 12px;
  line-height: 1.6;
}
.comic-state-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 8px;
  margin-top: 6px;
}
.spin {
  animation: comic-spin 0.9s linear infinite;
}
@keyframes comic-spin {
  to {
    transform: rotate(360deg);
  }
}
@media (max-width: 760px) {
  .comic-stage {
    gap: 4px;
  }
  .comic-page-image {
    width: 100%;
  }
  .comic-single-page img {
    max-height: 100dvh;
    cursor: default;
    pointer-events: none;
  }
  .comic-single-page .comic-paged-image {
    cursor: default;
    pointer-events: none;
  }
}
</style>
