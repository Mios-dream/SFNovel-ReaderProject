<script setup lang="ts">
import { computed, inject, nextTick, onMounted, ref, watch } from "vue";
import {
  ChevronLeft,
  ChevronRight,
  Columns2,
  EyeOff,
  Images,
  ListTree,
  LoaderCircle,
  MapPin,
  ScrollText,
  X,
  ZoomIn,
  ZoomOut,
} from "lucide-vue-next";
import { useRouter } from "vue-router";
import { deskInjectionKey } from "../deskContext";

function requireDesk() {
  const desk = inject(deskInjectionKey);
  if (!desk) throw new Error("Desk context is unavailable");
  return desk;
}

type DisplayMode = "vertical" | "paged";

const desk = requireDesk();
const router = useRouter();
const readerElement = ref<HTMLElement>();
const pageViewport = ref<HTMLElement>();
const book = computed(() => desk.localBook.value);
const online = computed(() => desk.readerSource.value === "online");
const comicChapter = computed(() =>
  online.value ? desk.comicChapter.value : desk.localComicChapter.value,
);
const bookName = computed(() =>
  online.value
    ? desk.onlineBook.value?.novelName || "在线漫画"
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
    return desk.comicChapters.value.map((chapter) => ({
      id: chapter.id,
      title: chapter.title,
      isVip: chapter.isVip,
    }));
  return (book.value?.comicChapters || []).map((chapter) => ({
    id: chapter.id,
    title: chapter.title,
    cover: chapter.cover,
  }));
});
const chapterIndex = computed(() =>
  chapters.value.findIndex((item) => item.id === comicChapter.value?.id),
);
const previousChapter = computed(() => chapters.value[chapterIndex.value - 1]);
const nextChapter = computed(() => chapters.value[chapterIndex.value + 1]);
const controlsVisible = ref(true);
const directoryOpen = ref(false);
const displayMode = ref<DisplayMode>("vertical");
const pageIndex = ref(0);
const chapterLoading = ref(false);
const pageCount = computed(() => comicChapter.value?.pages.length || 0);
const requestedChapterId = ref<number>();
const onlineError = computed(() =>
  online.value ? desk.chapterError.value || desk.directoryError.value : "",
);
const onlineLoading = computed(
  () =>
    online.value && (desk.chapterLoading.value || desk.directoryLoading.value),
);
const onlineNeedsAuth = computed(
  () => online.value && !desk.auth.value.webAuthenticated,
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
    if (online.value) await desk.openOnlineComicChapter(chapterId);
    else await desk.openLocalComicChapter(chapterId);
    if (comicChapter.value?.id !== chapterId) return;
    pageIndex.value = showLastPage
      ? Math.max(0, (comicChapter.value.pages.length || 1) - 1)
      : 0;
    resetImageView();
    resetCarouselPosition();
    directoryOpen.value = false;
    await nextTick();
    readerElement.value?.scrollTo({ top: 0 });
  } finally {
    chapterLoading.value = false;
  }
}

function goBack() {
  if (online.value) {
    desk.backFromOnlineReading();
    return;
  }
  desk.navigate("libraryDetail");
  void router.push(`/library/${encodeURIComponent(bookName.value)}`);
}

async function retryOnlineLoad() {
  try {
    await desk.loadDirectory(true);
    const targetId =
      requestedChapterId.value ||
      desk.pendingChapterId.value ||
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
  if (
    displayMode.value === "vertical" &&
    element &&
    element.scrollTop + element.clientHeight >= element.scrollHeight - 32
  ) {
    void openChapter(nextChapter.value?.id);
  }
}

function setDisplayMode(mode: DisplayMode) {
  displayMode.value = mode;
  resetImageView();
  controlsVisible.value = mode === "vertical";
  void nextTick(() => {
    readerElement.value?.scrollTo({ top: 0 });
  });
}

function handleReaderClick(event: MouseEvent) {
  if (ignoreNextReaderClick) {
    ignoreNextReaderClick = false;
    return;
  }
  const rect = readerElement.value?.getBoundingClientRect();
  if (!rect) return;
  const localY = event.clientY - rect.top;
  if (localY < rect.height * 0.25 || localY > rect.height * 0.75) return;
  controlsVisible.value = !controlsVisible.value;
}

function handleFlipPointerDown(event: PointerEvent) {
  if (displayMode.value !== "paged" || carouselTransitioning.value) return;
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

function showDirectory() {
  directoryOpen.value = true;
}

onMounted(async () => {
  if (online.value) {
    if (!desk.onlineBook.value) {
      void router.replace("/remote-book");
      return;
    }
    try {
      await desk.loadDirectory();
      const targetId =
        desk.comicChapter.value?.id ||
        desk.pendingChapterId.value ||
        chapters.value[0]?.id;
      if (targetId && desk.comicChapter.value?.id !== targetId)
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
  desk.navigate("comicReader");
});

watch(
  () => desk.pendingChapterId.value,
  (chapterId) => {
    if (
      online.value &&
      chapterId &&
      desk.comicChapter.value?.id !== chapterId
    ) {
      void openChapter(chapterId);
    }
  },
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
    <header
      v-show="controlsVisible"
      class="comic-floating-bar comic-top-bar"
      @click.stop
    >
      <button
        class="icon-button"
        title="返回书籍详情"
        aria-label="返回书籍详情"
        @click="goBack"
      >
        <ChevronLeft :size="21" />
      </button>
      <div class="comic-heading">
        <small>第 {{ chapterIndex + 1 }} 章</small
        ><strong>{{ comicChapter.title }}</strong>
      </div>
      <button
        class="icon-button"
        title="漫画目录"
        aria-label="漫画目录"
        @click="showDirectory"
      >
        <ListTree :size="20" />
      </button>
    </header>

    <div v-if="onlineError" class="comic-error-banner" @click.stop>
      <span>{{ onlineError }}</span>
      <button
        v-if="onlineNeedsAuth"
        type="button"
        @click="desk.credentialsOpen.value = true"
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
        <header class="comic-page-chrome comic-page-topbar" @click.stop>
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
          class="comic-page-chrome comic-page-bottom-meta"
          aria-label="书名和页数"
        >
          <span>{{ bookName }}</span>
          <span>{{ pageIndex + 1 }} / {{ pageCount }}</span>
        </div>
      </div>
    </main>

    <footer
      v-show="controlsVisible"
      class="comic-floating-bar comic-bottom-bar"
      @click.stop
    >
      <button
        class="icon-button"
        title="上一章或上一页"
        aria-label="上一章或上一页"
        :disabled="
          !previousChapter && (displayMode === 'vertical' || pageIndex === 0)
        "
        @click="previous"
      >
        <ChevronLeft :size="21" />
      </button>
      <button
        class="icon-button mode-toggle"
        :title="
          displayMode === 'vertical' ? '切换到翻页显示' : '切换到竖屏显示'
        "
        :aria-label="
          displayMode === 'vertical' ? '切换到翻页显示' : '切换到竖屏显示'
        "
        @click="
          setDisplayMode(displayMode === 'vertical' ? 'paged' : 'vertical')
        "
      >
        <ScrollText v-if="displayMode === 'vertical'" :size="18" />
        <Columns2 v-else :size="18" />
      </button>
      <button
        class="icon-button"
        title="隐藏阅读菜单"
        aria-label="隐藏阅读菜单"
        @click="controlsVisible = false"
      >
        <EyeOff :size="19" />
      </button>
      <button
        v-if="displayMode === 'paged'"
        class="icon-button"
        title="放大图片"
        aria-label="放大图片"
        :disabled="imageScale >= 3"
        @click="zoomImage(0.25)"
      >
        <ZoomIn :size="19" />
      </button>
      <button
        v-if="displayMode === 'paged'"
        class="icon-button"
        title="缩小图片"
        aria-label="缩小图片"
        :disabled="imageScale <= 1"
        @click="zoomImage(-0.25)"
      >
        <ZoomOut :size="19" />
      </button>
      <button
        class="icon-button"
        title="下一章或下一页"
        aria-label="下一章或下一页"
        :disabled="
          !nextChapter &&
          (displayMode === 'vertical' || pageIndex === pageCount - 1)
        "
        @click="next"
      >
        <ChevronRight :size="21" />
      </button>
    </footer>
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
        @click="desk.credentialsOpen.value = true"
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

  <Transition name="directory-drawer">
    <div
      v-if="directoryOpen"
      class="directory-backdrop"
      @click.self="directoryOpen = false"
    >
      <section
        class="directory-panel"
        role="dialog"
        aria-modal="true"
        aria-label="漫画目录"
      >
        <header class="directory-header">
          <div class="directory-title">
            <small>{{ bookName }}</small>
            <h2>章节目录</h2>
          </div>
          <div class="directory-header-actions">
            <span v-if="chapters.length" class="directory-count"
              >{{ chapters.length }} 章</span
            >
            <button
              class="directory-close"
              type="button"
              title="关闭目录"
              aria-label="关闭目录"
              @click="directoryOpen = false"
            >
              <X :size="19" />
            </button>
          </div>
        </header>
        <div v-if="desk.directoryLoading.value" class="directory-state">
          <LoaderCircle class="spin" :size="24" />正在读取目录
        </div>
        <div
          v-else-if="desk.directoryError.value"
          class="directory-state error"
        >
          {{ desk.directoryError.value }}
          <button type="button" @click="retryOnlineLoad">重试</button>
        </div>
        <div v-else-if="chapters.length" class="directory-list">
          <section class="directory-volume">
            <h3>全部章节</h3>
            <button
              v-for="item in chapters"
              :key="item.id"
              class="directory-item directory-chapter"
              :class="{ active: item.id === comicChapter?.id }"
              @click="openChapter(item.id)"
            >
              <img
                v-if="item.cover"
                class="directory-cover"
                :src="item.cover"
                :alt="`${item.title} 首图`"
              />
              <strong>{{ item.title }}</strong>
              <span class="directory-row-meta">
                <small v-if="item.isVip" class="vip-badge">VIP</small>
                <MapPin
                  v-if="item.id === comicChapter?.id"
                  class="directory-current-icon"
                  :size="16"
                  aria-hidden="true"
                />
              </span>
            </button>
          </section>
        </div>
        <div v-else class="directory-state">暂无目录内容</div>
      </section>
    </div>
  </Transition>
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
.comic-floating-bar {
  position: fixed;
  z-index: 30;
  display: flex;
  align-items: center;
  min-height: 52px;
  background: white;
  box-shadow: 0 5px 20px rgba(89, 57, 48, 0.12);
  backdrop-filter: blur(14px);
}
.comic-top-bar {
  /* top: max(12px, env(safe-area-inset-top)); */
  right: 0;
  left: 0;
  justify-content: space-between;
  padding: 5px;
  padding-top: max(12px, env(safe-area-inset-top));
}
.comic-bottom-bar {
  right: 0;
  left: 0;
  /* bottom: max(14px, env(safe-area-inset-bottom)); */
  bottom: 0;
  justify-content: center;
  gap: 8px;
  padding: 5px;
  padding-bottom: max(14px, env(safe-area-inset-bottom));
  border-radius: 20px 20px 0 0;
}
.comic-heading {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
}
.comic-heading small {
  flex: 0 0 auto;
  color: var(--theme-color-dark);
  font-size: 14px;
}
.comic-heading strong {
  overflow: hidden;
  color: var(--theme-color-dark);
  font-size: 14px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.icon-button {
  display: inline-flex;
  width: 40px;
  height: 40px;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 5px;
  color: var(--theme-color-dark);
  background: transparent;
}
.icon-button:hover,
.icon-button.active {
  color: var(--theme-color-dark);
  background: var(--theme-color-light);
}
.icon-button:disabled {
  opacity: 0.32;
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
  pointer-events: auto;
}
.comic-page-topbar {
  top: max(8px, env(safe-area-inset-top));
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
}
.comic-page-back:hover {
  background: var(--theme-color-light);
}
.comic-page-bottom-meta {
  position: fixed;
  right: 0;
  bottom: 0;
  display: flex;
  max-width: min(60%, 360px);
  gap: 8px;
  align-items: center;
  color: white;
  font-size: 11px;
  line-height: 1.2;
  padding: 4px 8px;
  background: rgba(0, 0, 0, 0.5);
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

.comic-single-page span {
  color: white;
  font-size: 12px;
}
.directory-backdrop {
  position: fixed;
  z-index: 20;
  inset: 0;
  display: flex;
  justify-content: flex-start;
  background: rgba(43, 36, 31, 0.46);
  backdrop-filter: blur(3px);
  -webkit-backdrop-filter: blur(3px);
}
.directory-panel {
  display: flex;
  width: min(350px, 70vw);
  height: 100%;
  flex-direction: column;
  overflow: hidden;
  border-right: 1px solid #eaded1;
  border-radius: 0 22px 22px 0;
  background: #fffdf8;
  box-shadow: 20px 0 48px rgba(46, 40, 33, 0.24);
}
.directory-header {
  display: flex;
  min-height: 78px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  padding: max(18px, env(safe-area-inset-top)) 18px 15px 20px;
  border-bottom: 1px solid #eee8dc;
  background: linear-gradient(180deg, #fffaf3 0%, #fffdf8 100%);
}
.directory-title {
  min-width: 0;
  flex: 1 1 auto;
}
.directory-title small {
  display: block;
  overflow: hidden;
  color: #b59885;
  font: 10px monospace;
  letter-spacing: 0.08em;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.directory-title h2 {
  margin: 5px 0 0;
  overflow: hidden;
  color: #443e36;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 22px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.directory-header-actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 10px;
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
.directory-close {
  display: inline-flex;
  width: 34px;
  height: 34px;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 50%;
  color: #79695e;
  background: #f4eee4;
  transition:
    color 0.18s ease,
    background 0.18s ease;
}
.directory-close:hover {
  color: #fff;
  background: var(--theme-color);
}
.directory-list {
  min-height: 0;
  flex: 1 1 auto;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding-bottom: env(safe-area-inset-bottom);
  scrollbar-color: #e7cbbb transparent;
  scrollbar-width: thin;
}
.directory-item {
  display: flex;
  width: 100%;
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
.directory-volume {
  padding: 8px 0 10px;
}
.directory-volume h3 {
  margin: 0;
  padding: 14px 20px 8px;
  color: #887f70;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.02em;
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
.directory-drawer-enter-active,
.directory-drawer-leave-active {
  transition: opacity 0.22s ease;
}
.directory-drawer-enter-active .directory-panel,
.directory-drawer-leave-active .directory-panel {
  transition: transform 0.26s cubic-bezier(0.22, 0.8, 0.24, 1);
}
.directory-drawer-enter-from,
.directory-drawer-leave-to {
  opacity: 0;
}
.directory-drawer-enter-from .directory-panel,
.directory-drawer-leave-to .directory-panel {
  transform: translateX(-100%);
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
.comic-state-actions button,
.directory-state button {
  flex: 0 0 auto;
  padding: 6px 10px;
  border: 1px solid rgba(214, 143, 113, 0.56);
  border-radius: 6px;
  color: var(--theme-color-dark);
  background: #fffaf7;
  font-size: 12px;
}
.comic-error-banner button:hover,
.comic-state-actions button:hover,
.directory-state button:hover {
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
.directory-state {
  display: flex;
  min-height: 0;
  flex: 1 1 auto;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 28px;
  color: #7f6f68;
  font-size: 13px;
  text-align: center;
}
.directory-state.error {
  flex-direction: column;
  color: #a4503a;
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
