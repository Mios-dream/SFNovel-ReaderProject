<script setup lang="ts">
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { PageFlip } from "page-flip";
import {
  computed,
  inject,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
} from "vue";
import {
  ChevronLeft,
  ChevronRight,
  Columns2,
  EyeOff,
  ListTree,
  ScrollText,
  X,
  MapPin,
} from "lucide-vue-next";
import { useRouter } from "vue-router";
import { deskInjectionKey } from "../deskContext";

function requireDesk() {
  const desk = inject(deskInjectionKey);
  if (!desk) throw new Error("Desk context is unavailable");
  return desk;
}

type DisplayMode = "vertical" | "paged";
type ChapterPart =
  | { type: "text"; value: string }
  | { type: "image"; alt: string; src: string };

const desk = requireDesk();
const router = useRouter();
const readerElement = ref<HTMLElement>();
const stageElement = ref<HTMLElement>();
const pageFlipHost = ref<HTMLElement>();
const pageMeasure = ref<HTMLElement>();
const chapter = computed(() => desk.localChapter.value);
const book = computed(() => desk.localBook.value);
const bookName = computed(() => book.value?.name || "本地阅读");
const imageDirectory = computed(() => book.value?.imageDirectory || "");
const chapters = computed(() =>
  (book.value?.chapterVolumes || []).flatMap((volume) =>
    volume.chapters.map((item) => ({ ...item, volume: volume.volume })),
  ),
);
const chapterIndex = computed(() =>
  chapters.value.findIndex((item) => item.id === chapter.value?.id),
);
const previousChapter = computed(() => chapters.value[chapterIndex.value - 1]);
const nextChapter = computed(() => chapters.value[chapterIndex.value + 1]);
const controlsVisible = ref(true);
const directoryOpen = ref(false);
const displayMode = ref<DisplayMode>("vertical");
const chapterLoading = ref(false);
const flipPageIndex = ref(0);
const flipPageCount = ref(0);
let pageFlip: PageFlip | undefined;
let resizeObserver: ResizeObserver | undefined;
let rebuildTimer: number | undefined;
let initialFlipPage = 0;
let openingNextChapter = false;
let pageFlipInitializing = false;
let flipPointerStart: { x: number; y: number } | undefined;
let ignoreNextReaderClick = false;

function chapterImageSource(relativePath: string) {
  const fileName = relativePath.slice("imgs/".length).replace(/\\/g, "/");
  const separator = imageDirectory.value.includes("\\") ? "\\" : "/";
  const imagePath = `${imageDirectory.value.replace(/[\\/]+$/, "")}${separator}${fileName.replace(/\//g, separator)}`;
  return isTauri() ? convertFileSrc(imagePath) : imagePath;
}

const chapterParts = computed<ChapterPart[]>(() => {
  const content = chapter.value?.content.replace(/^##\s+.+\r?\n+/, "") || "";
  const pattern = /!\[([^\]]*)\]\((imgs[\\/][^)]+)\)/g;
  const parts: ChapterPart[] = [];
  let lastIndex = 0;
  for (const match of content.matchAll(pattern)) {
    const index = match.index || 0;
    if (index > lastIndex)
      parts.push({ type: "text", value: content.slice(lastIndex, index) });
    parts.push({
      type: "image",
      alt: match[1] || "章节插图",
      src: chapterImageSource(match[2]),
    });
    lastIndex = index + match[0].length;
  }
  if (lastIndex < content.length)
    parts.push({ type: "text", value: content.slice(lastIndex) });
  return parts;
});

async function openChapter(chapterId?: number, startAtEnd = false) {
  if (!chapterId || chapterLoading.value) return;
  chapterLoading.value = true;
  try {
    await desk.openLocalChapter(chapterId);
    directoryOpen.value = false;
    await nextTick();
    readerElement.value?.scrollTo({ top: 0, left: 0 });
    initialFlipPage = startAtEnd ? Number.MAX_SAFE_INTEGER : 0;
    if (displayMode.value === "paged") await initializePageFlip();
  } finally {
    chapterLoading.value = false;
  }
}

function goBack() {
  desk.navigate("libraryDetail");
  void router.push(`/library/${encodeURIComponent(bookName.value)}`);
}

function previous() {
  if (displayMode.value === "paged" && pageFlip && flipPageIndex.value > 0) {
    // Use the same corner for both directions so the curl geometry is identical.
    pageFlip.flipPrev("top");
    return;
  }
  void openChapter(previousChapter.value?.id, displayMode.value === "paged");
}

function next() {
  if (
    displayMode.value === "paged" &&
    pageFlip &&
    flipPageIndex.value < flipPageCount.value - 1
  ) {
    pageFlip.flipNext("top");
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

function createFlipPage(measure: HTMLElement) {
  const page = document.createElement("section");
  const body = document.createElement("div");
  page.className = "reader-flip-page";
  body.className = "reader-flip-page-body";
  page.appendChild(body);
  measure.replaceChildren(page);
  return { page, body };
}

function imageReady(image: HTMLImageElement) {
  if (image.complete) return image.decode().catch(() => undefined);
  return new Promise<void>((resolve) => {
    image.addEventListener("load", () => resolve(), { once: true });
    image.addEventListener("error", () => resolve(), { once: true });
  });
}

async function createFlipPages(width: number, height: number) {
  const measure = pageMeasure.value;
  if (!measure) return [];
  measure.style.width = `${width}px`;
  measure.style.height = `${height}px`;
  const pages: HTMLElement[] = [];
  let current: { page: HTMLElement; body: HTMLElement } | undefined;

  function startPage() {
    current = createFlipPage(measure!);
    pages.push(current.page);
    return current;
  }

  const titlePage = startPage();
  const title = document.createElement("h1");
  title.className = "reader-flip-heading";
  title.textContent = chapter.value?.title || "本章";
  titlePage.body.appendChild(title);
  current = titlePage;

  for (const part of chapterParts.value) {
    if (part.type === "image") {
      const page: { page: HTMLElement; body: HTMLElement } =
        current || startPage();
      const image = document.createElement("img");
      image.className = "reader-flip-image";
      image.src = part.src;
      image.alt = part.alt;
      page.body.appendChild(image);
      await imageReady(image);
      // Keep an illustration with adjacent text whenever the page has room.
      // If it would overflow a page that already contains text, move it to a
      // fresh page and leave that page available for the following text.
      if (
        page.body.scrollHeight > page.body.clientHeight + 1 &&
        page.body.childElementCount > 1
      ) {
        image.remove();
        const nextPage = startPage();
        nextPage.body.appendChild(image);
        await imageReady(image);
        current = nextPage;
      } else {
        current = page;
      }
      continue;
    }

    const characters = Array.from(part.value);
    let offset = 0;
    while (offset < characters.length) {
      const page = current || startPage();
      const paragraph = document.createElement("p");
      paragraph.className = "reader-flip-text";
      page.body.appendChild(paragraph);

      let low = 1;
      let high = characters.length - offset;
      let best = 0;
      while (low <= high) {
        const middle = Math.floor((low + high) / 2);
        paragraph.textContent = characters
          .slice(offset, offset + middle)
          .join("");
        if (page.body.scrollHeight <= page.body.clientHeight + 1) {
          best = middle;
          low = middle + 1;
        } else {
          high = middle - 1;
        }
      }

      if (!best) {
        paragraph.remove();
        if (page.body.childElementCount) {
          current = undefined;
          continue;
        }
        paragraph.textContent = characters[offset];
        page.body.appendChild(paragraph);
        best = 1;
      } else {
        paragraph.textContent = characters
          .slice(offset, offset + best)
          .join("");
      }
      offset += best;
      if (offset < characters.length) current = undefined;
    }
  }

  if (!pages.length) startPage();
  return pages;
}

function clearPageFlip() {
  window.clearTimeout(rebuildTimer);
  if (!pageFlip) return;
  pageFlip.getUI().destroy();
  pageFlip = undefined;
}

async function initializePageFlip() {
  if (displayMode.value !== "paged" || pageFlipInitializing) return;
  pageFlipInitializing = true;
  try {
    await nextTick();
    await document.fonts?.ready;
    if (displayMode.value !== "paged") return;
    const host = pageFlipHost.value;
    if (!host) return;
    const { width, height } = host.getBoundingClientRect();
    if (width < 120 || height < 160) return;

    clearPageFlip();
    const pages = await createFlipPages(Math.floor(width), Math.floor(height));
    if (displayMode.value !== "paged" || !pageFlipHost.value) return;

    const readerPageCount = pages.length;
    if (nextChapter.value) {
      const transition = document.createElement("section");
      transition.className = "reader-flip-page reader-flip-transition";
      transition.textContent = "下一章";
      pages.push(transition);
    }
    const startPage = Math.min(initialFlipPage, readerPageCount - 1);
    pageFlip = new PageFlip(host, {
      width: Math.floor(width),
      height: Math.floor(height),
      size: "fixed",
      startPage,
      drawShadow: true,
      flippingTime: 560,
      usePortrait: true,
      autoSize: false,
      maxShadowOpacity: 0.34,
      mobileScrollSupport: false,
      swipeDistance: 28,
      clickEventForward: false,
      useMouseEvents: true,
      showPageCorners: true,
    });
    pageFlip.on("flip", (event) => {
      const index = Number(event.data);
      flipPageIndex.value = index;
      flipPageCount.value = pageFlip?.getPageCount() || 0;
      if (
        index === readerPageCount &&
        nextChapter.value &&
        !openingNextChapter
      ) {
        openingNextChapter = true;
        window.setTimeout(() => {
          openingNextChapter = false;
          void openChapter(nextChapter.value?.id);
        }, 120);
      }
    });
    pageFlip.loadFromHTML(pages);
    flipPageIndex.value = startPage;
    flipPageCount.value = pageFlip.getPageCount();
    initialFlipPage = 0;
  } finally {
    pageFlipInitializing = false;
  }
}

function setDisplayMode(mode: DisplayMode) {
  if (displayMode.value === mode) return;
  if (mode === "vertical") clearPageFlip();
  displayMode.value = mode;
  controlsVisible.value = mode === "vertical";
  void nextTick(() => {
    readerElement.value?.scrollTo({ top: 0, left: 0 });
    if (mode === "paged") void initializePageFlip();
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
  if (displayMode.value !== "paged") return;
  flipPointerStart = { x: event.clientX, y: event.clientY };
}

function handleFlipPointerUp(event: PointerEvent) {
  const start = flipPointerStart;
  flipPointerStart = undefined;
  if (!start || displayMode.value !== "paged") return;
  const deltaX = event.clientX - start.x;
  const deltaY = Math.abs(event.clientY - start.y);
  if (Math.abs(deltaX) < 36 || Math.abs(deltaX) < deltaY * 1.2) return;
  ignoreNextReaderClick = true;
  window.setTimeout(() => {
    ignoreNextReaderClick = false;
  }, 350);
}

function showDirectory() {
  directoryOpen.value = true;
}

onMounted(() => {
  if (!book.value || !chapter.value) void router.replace("/library");
  else {
    desk.navigate("reader");
    resizeObserver = new ResizeObserver(() => {
      if (displayMode.value !== "paged") return;
      window.clearTimeout(rebuildTimer);
      rebuildTimer = window.setTimeout(() => void initializePageFlip(), 180);
    });
    if (stageElement.value) resizeObserver.observe(stageElement.value);
  }
});

onBeforeUnmount(() => {
  clearPageFlip();
  resizeObserver?.disconnect();
  directoryOpen.value = false;
});
</script>

<template>
  <article
    v-if="chapter"
    ref="readerElement"
    class="reader"
    :class="{ 'paged-reader': displayMode === 'paged' }"
    @scroll="handleReaderScroll"
    @click="handleReaderClick"
  >
    <header
      v-show="controlsVisible"
      class="reader-floating-bar reader-top-bar"
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
      <div class="reader-heading">
        <small>{{ chapter.volume }}</small
        ><strong>{{ chapter.title }}</strong>
      </div>
      <button
        class="icon-button"
        title="章节目录"
        aria-label="章节目录"
        @click="showDirectory"
      >
        <ListTree :size="20" />
      </button>
    </header>

    <main
      ref="stageElement"
      class="reader-stage"
      :class="{ paged: displayMode === 'paged' }"
    >
      <div v-if="displayMode === 'vertical'" class="chapter-content">
        <h1 class="chapter-title">{{ chapter.title }}</h1>
        <template v-for="(part, index) in chapterParts" :key="index">
          <img
            v-if="part.type === 'image'"
            class="chapter-image"
            :src="part.src"
            :alt="part.alt"
          />
          <p v-else class="chapter-text">{{ part.value }}</p>
        </template>
      </div>
      <div
        v-else
        class="page-flip-frame"
        @pointerdown="handleFlipPointerDown"
        @pointerup="handleFlipPointerUp"
        @pointercancel="flipPointerStart = undefined"
      >
        <header class="page-flip-chrome page-flip-topbar" @click.stop>
          <button
            class="page-flip-back"
            title="返回书籍详情"
            aria-label="返回书籍详情"
            @click="goBack"
          >
            <ChevronLeft :size="26" />
          </button>
          <strong>{{ chapter.title }}</strong>
        </header>
        <div ref="pageFlipHost" class="page-flip-host" />
        <div
          class="page-flip-chrome page-flip-bottom-meta"
          aria-label="书名和页数"
        >
          <span>{{ bookName }}</span>
          <span
            >{{ flipPageIndex + 1 }} /
            {{ Math.max(flipPageCount - (nextChapter ? 1 : 0), 1) }}</span
          >
        </div>
      </div>
    </main>

    <div ref="pageMeasure" class="page-measure" aria-hidden="true" />

    <footer
      v-show="controlsVisible"
      class="reader-floating-bar reader-bottom-bar"
      @click.stop
    >
      <button
        class="icon-button"
        title="上一章或上一页"
        aria-label="上一章或上一页"
        :disabled="
          displayMode === 'vertical'
            ? !previousChapter
            : !previousChapter && flipPageIndex === 0
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
        class="icon-button"
        title="下一章或下一页"
        aria-label="下一章或下一页"
        :disabled="
          displayMode === 'vertical'
            ? !nextChapter
            : !nextChapter && flipPageIndex >= flipPageCount - 1
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
      aria-label="小说目录"
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
      <section
        v-for="volume in book?.chapterVolumes"
        :key="volume.volume"
        class="directory-volume"
      >
        <h3>{{ volume.volume }}</h3>
        <button
          v-for="item in volume.chapters"
          :key="item.id"
          class="directory-chapter"
          :class="{ active: item.id === chapter?.id }"
          @click="openChapter(item.id)"
        >
          <MapPin
            v-if="item.id === chapter?.id"
            class="directory-current-icon"
            :size="17"
            aria-hidden="true"
          />
          {{ item.title }}
        </button>
      </section>
    </section>
  </div>
</template>

<style scoped>
.reader {
  position: fixed;
  inset: 0;
  overflow-y: auto;
  overflow-x: hidden;
  background: var(--background-color);
  color: var(--ink);
}
.reader-stage {
  width: min(820px, 100%);
  min-height: 100%;
  margin: 0 auto;
  /* padding: 90px 24px 104px; */
}
.chapter-content {
  padding: 30px clamp(18px, 5vw, 58px);
  border: 1px solid rgba(224, 176, 154, 0.56);
  border-radius: 8px;
  background: rgba(255, 250, 247, 0.76);
  box-shadow: 0 12px 28px rgba(137, 76, 55, 0.08);
  font-family: "Microsoft YaHei", sans-serif;
  font-size: 16px;
  line-height: 2;
}
.chapter-text {
  margin: 0;
  white-space: pre-wrap;
}
.chapter-image {
  display: block;
  width: auto;
  max-width: 100%;
  max-height: 80vh;
  margin: 20px auto;
  object-fit: contain;
}
.reader-floating-bar {
  position: fixed;
  z-index: 30;
  display: flex;
  align-items: center;
  min-height: 52px;
  background: white;
}
.reader-top-bar {
  left: 0;
  right: 0;
  justify-content: space-between;
  padding: 5px;
  padding-top: max(12px, env(safe-area-inset-top));
  box-shadow: 0 5px 20px rgba(89, 57, 48, 0.12);
}
.reader-bottom-bar {
  bottom: 0;
  left: 0;
  right: 0;
  justify-content: center;
  gap: 8px;
  padding: 5px;
  padding-bottom: max(14px, env(safe-area-inset-bottom));
  border-radius: 20px 20px 0 0;
  box-shadow: 0 -5px 20px rgba(89, 57, 48, 0.12);
}
.reader-heading {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
  color: var(--theme-color-dark);
}
.reader-heading small {
  flex: 0 0 auto;
  font-size: 14px;
}
.reader-heading strong {
  overflow: hidden;
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
.paged-reader {
  overflow: hidden;
}
.reader-stage.paged {
  display: flex;
  width: 100%;
  height: 100dvh;
  min-height: 100dvh;
  align-items: center;
  justify-content: center;
  padding: 0;
  overflow: hidden;
}
.page-flip-frame {
  position: relative;
  display: flex;
  width: 100%;
  height: 100%;
  align-items: center;
  justify-content: center;
  touch-action: pan-y;
}
.page-flip-chrome {
  position: absolute;
  z-index: 20;
  color: var(--theme-color-dark);
  pointer-events: auto;
}
.page-flip-topbar {
  top: max(12px, env(safe-area-inset-top));
  right: 0;
  left: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 42px;
  padding: 0 54px;
}
.page-flip-topbar strong {
  max-width: min(72%, 680px);
  overflow: hidden;
  font-size: 14px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.page-flip-back {
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
.page-flip-back:hover {
  background: var(--theme-color-light);
}
.page-flip-bottom-meta {
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
.page-flip-bottom-meta span:first-child {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.page-flip-host {
  width: min(100%, 820px);
  height: 100%;
}
.page-measure {
  position: fixed;
  top: 0;
  left: -10000px;
  visibility: hidden;
  overflow: hidden;
  pointer-events: none;
}
:global(.reader-flip-page) {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: var(--background-color);
  color: var(--ink);
}
:global(.reader-flip-page-body) {
  display: flex;
  position: relative;
  width: 100%;
  height: 100%;
  flex-direction: column;
  justify-content: flex-start;
  padding: 76px clamp(24px, 7vw, 72px) 90px;
  overflow: hidden;
  font-family: "Microsoft YaHei", sans-serif;
  font-size: 16px;
  line-height: 2;
}
:global(.reader-flip-text) {
  flex: 0 0 auto;
  margin: 0;
  white-space: pre-wrap;
}
:global(.reader-flip-heading) {
  flex: 0 0 auto;
  margin: 0 0 24px;
  color: var(--ink);
  font-size: clamp(22px, 4vw, 32px);
  font-weight: 700;
  line-height: 1.35;
  text-align: left;
}
:global(.reader-flip-image) {
  display: block;
  flex: 0 1 auto;
  width: auto;
  max-width: 100%;
  max-height: 50%;
  margin: 12px auto;
  object-fit: contain;
}
:global(.reader-flip-transition) {
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--theme-color-dark);
  font-size: 16px;
  font-weight: 600;
}
.chapter-title {
  margin: 0 0 26px;
  color: var(--ink);
  font-size: clamp(24px, 5vw, 34px);
  line-height: 1.35;
}
.directory-backdrop {
  position: fixed;
  z-index: 20;
  inset: 0;
  display: flex;
  justify-content: flex-start;
  background: rgba(42, 34, 37, 0.45);
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
  padding: max(10px, env(safe-area-inset-top)) 14px 0;
  border-bottom: 1px solid #ececec;
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
  padding: 0;
  font-family: KaTongFont;
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
.directory-volume {
  padding: 13px 0 4px;
}
.directory-volume h3 {
  margin: 0;
  padding: 0 20px 8px;
  color: #242424;
  font-size: 16px;
}
.directory-chapter {
  display: flex;
  width: 100%;
  min-height: 48px;
  align-items: center;
  gap: 7px;
  padding: 12px 20px 12px 34px;
  border: 0;
  border-top: 1px solid #eeeeee;
  color: #3f3f3f;
  background: transparent;
  text-align: left;
}
.directory-chapter:hover {
  background: #f7f7f7;
}
.directory-chapter.active {
  color: #e64e36;
  /* background: var(--theme-color-light); */
  font-weight: 600;
}
.directory-current-icon {
  flex: 0 0 auto;
}

@media (max-width: 760px) {
  .chapter-content {
    border: 0;
    background: none;
    box-shadow: none;
  }
  .chapter-image {
    max-height: none;
  }

  .reader-stage.paged {
    height: 100dvh;
    min-height: 100dvh;
    padding-left: 0;
    padding-right: 0;
  }
  :global(.reader-flip-page-body) {
    padding: calc(66px + env(safe-area-inset-top)) 12px
      env(safe-area-inset-bottom);
    font-size: 15px;
  }
  .page-flip-topbar {
    top: max(8px, env(safe-area-inset-top));
  }
  .page-flip-bottom-meta {
    right: max(10px, env(safe-area-inset-right));
    bottom: max(12px, env(safe-area-inset-bottom));
    max-width: 62%;
  }
}
</style>
