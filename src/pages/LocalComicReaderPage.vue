<script setup lang="ts">
import { computed, inject, nextTick, onMounted, ref } from "vue";
import {
  ChevronLeft,
  ChevronRight,
  Columns2,
  EyeOff,
  Images,
  ListTree,
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
const comicChapter = computed(() => desk.localComicChapter.value);
const book = computed(() => desk.localBook.value);
const bookName = computed(() => book.value?.name || "本地漫画");
const chapters = computed(() => book.value?.comicChapters || []);
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
  try {
    const chapter = await desk.openLocalComicChapter(chapterId);
    pageIndex.value = showLastPage
      ? Math.max(0, (chapter?.pages.length || 1) - 1)
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
  desk.navigate("libraryDetail");
  void router.push(`/library/${encodeURIComponent(bookName.value)}`);
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

onMounted(() => {
  if (!book.value || !comicChapter.value) void router.replace("/library");
  else desk.navigate("comicReader");
});
</script>

<template>
  <article
    v-if="comicChapter"
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

    <main class="comic-stage" :class="{ paged: displayMode === 'paged' }">
      <template v-if="displayMode === 'vertical'">
        <img
          v-for="(page, index) in comicChapter.pages"
          :key="page"
          class="comic-page-image"
          :src="page"
          :alt="`第 ${index + 1} 页`"
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
        <button
          class="directory-close"
          type="button"
          title="关闭目录"
          aria-label="关闭目录"
          @click="directoryOpen = false"
        >
          <X :size="19" />
        </button>
      </header>
      <section class="directory-volume">
        <h3>全部章节</h3>
        <button
          v-for="item in chapters"
          :key="item.id"
          class="directory-item directory-chapter"
          :class="{ active: item.id === comicChapter?.id }"
          @click="openChapter(item.id)"
        >
          <MapPin
            v-if="item.id === comicChapter?.id"
            class="directory-current-icon"
            :size="17"
            aria-hidden="true"
          />
          <img
            v-if="item.cover"
            :src="item.cover"
            :alt="`${item.title} 首图`"
          /><Images v-else :size="24" />

          <strong>{{ item.title }}</strong>
        </button>
      </section>
    </section>
  </div>
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
.icon-button,
.directory-panel header button {
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
  background: rgba(16, 12, 25, 0.62);
}
.directory-panel {
  width: min(390px, 88vw);
  height: 100%;
  overflow-y: auto;
  background: #fff;
  box-shadow: 12px 0 36px rgba(54, 37, 39, 0.22);
}
.directory-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 62px;
  /* padding: max(10px, env(safe-area-inset-top)) 14px 0; */
  border-bottom: 1px solid #ececec;
  font-family: KaTongFont;
}
.directory-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 0;
  color: var(--theme-color-dark);
  background: transparent;
}
.directory-close {
  width: 38px;
  height: 38px;
}
.directory-close:hover {
  background: var(--theme-color-light);
}
.directory-title {
  padding: 18px 20px 12px;
}
.directory-panel small {
  color: #8a8a8a;
  font-size: 11px;
}
.directory-panel h2 {
  margin: 4px 0 0;
  color: #242424;
  font-size: 21px;
}
.directory-item {
  display: flex;
  width: 100%;
  min-height: 76px;
  align-items: center;
  gap: 12px;
  padding: 9px 30px;
  border: 0;
  border-bottom: 1px solid #eeeeee;
  color: #3f3f3f;
  background: transparent;
  text-align: left;
}
.directory-item:hover {
  background: #f7f7f7;
}
.directory-item.active {
  color: #e64e36;
  /* background: var(--theme-color-light); */
  font-weight: 600;
}
.directory-item img {
  width: 72px;
  height: 48px;
  flex: 0 0 auto;
  border-radius: 4px;
  object-fit: cover;
  border: 1px solid #eaeaea;
}
.directory-item strong {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.directory-current-icon {
  flex: 0 0 auto;
}
.directory-volume {
  padding: 13px 0 4px;
}
.directory-volume h3 {
  margin: 0;
  padding: 0 20px 8px;
  color: #242424;
  font-size: 16px;
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
