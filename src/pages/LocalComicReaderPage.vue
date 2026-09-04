<script setup lang="ts">
import { computed, inject, nextTick, onMounted, ref } from "vue";
import {
  ChevronLeft,
  ChevronRight,
  Columns2,
  EyeOff,
  Images,
  ListTree,
  ScrollText,
  X,
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
const currentPage = computed(() => comicChapter.value?.pages[pageIndex.value]);
const slideDirection = ref<"forward" | "back">("forward");
let flipPointerStart: { x: number; y: number } | undefined;
let ignoreNextReaderClick = false;

async function openChapter(chapterId?: number, showLastPage = false) {
  if (!chapterId || chapterLoading.value) return;
  chapterLoading.value = true;
  try {
    const chapter = await desk.openLocalComicChapter(chapterId);
    pageIndex.value = showLastPage
      ? Math.max(0, (chapter?.pages.length || 1) - 1)
      : 0;
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
    slideDirection.value = "back";
    pageIndex.value -= 1;
    return;
  }
  void openChapter(previousChapter.value?.id, displayMode.value === "paged");
}

function next() {
  if (displayMode.value === "paged" && pageIndex.value < pageCount.value - 1) {
    slideDirection.value = "forward";
    pageIndex.value += 1;
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
  if (deltaX < 0) next();
  else previous();
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
        @click="directoryOpen = true"
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
        @pointerup="handleFlipPointerUp"
        @pointercancel="flipPointerStart = undefined"
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
        <Transition :name="`comic-slide-${slideDirection}`" mode="out-in">
          <img
            v-if="currentPage"
            :key="currentPage"
            :src="currentPage"
            :alt="`第 ${pageIndex + 1} 页`"
          />
        </Transition>
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
      <div class="display-switch" aria-label="漫画显示模式">
        <button
          :class="{ active: displayMode === 'vertical' }"
          title="竖屏显示"
          aria-label="竖屏显示"
          @click="setDisplayMode('vertical')"
        >
          <ScrollText :size="18" />
        </button>
        <button
          :class="{ active: displayMode === 'paged' }"
          title="翻页显示"
          aria-label="翻页显示"
          @click="setDisplayMode('paged')"
        >
          <Columns2 :size="18" />
        </button>
      </div>
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
      <header>
        <div>
          <small>本地漫画</small>
          <h2>章节目录</h2>
        </div>
        <button
          title="关闭目录"
          aria-label="关闭目录"
          @click="directoryOpen = false"
        >
          <X :size="19" />
        </button>
      </header>
      <button
        v-for="item in chapters"
        :key="item.id"
        class="directory-item"
        :class="{ active: item.id === comicChapter?.id }"
        @click="openChapter(item.id)"
      >
        <img
          v-if="item.cover"
          :src="item.cover"
          :alt="`${item.title} 首图`"
        /><Images v-else :size="24" /><strong>{{ item.title }}</strong>
      </button>
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
  gap: 14px;
  margin: 0 auto;
}
.comic-page-image {
  display: block;
  width: min(100%, 760px);
  height: auto;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
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
.display-switch button,
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
.display-switch button:hover,
.display-switch button.active {
  color: var(--theme-color-dark);
  background: var(--theme-color-light);
}
.icon-button:disabled {
  opacity: 0.32;
}
.display-switch {
  display: flex;
  overflow: hidden;
  border: 1px solid var(--theme-color-light);
  border-radius: 6px;
  background: var(--background-color);
}
.display-switch button {
  border-radius: 0;
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
  touch-action: pan-y;
}
.comic-slide-forward-enter-active,
.comic-slide-forward-leave-active,
.comic-slide-back-enter-active,
.comic-slide-back-leave-active {
  transition:
    opacity 180ms ease,
    transform 180ms ease;
}
.comic-slide-forward-enter-from {
  opacity: 0;
  transform: translateX(18px);
}
.comic-slide-forward-leave-to {
  opacity: 0;
  transform: translateX(-18px);
}
.comic-slide-back-enter-from {
  opacity: 0;
  transform: translateX(-18px);
}
.comic-slide-back-leave-to {
  opacity: 0;
  transform: translateX(18px);
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
.comic-single-page span {
  color: white;
  font-size: 12px;
}
.directory-backdrop {
  position: fixed;
  z-index: 20;
  inset: 0;
  display: flex;
  justify-content: flex-end;
  background: rgba(16, 12, 25, 0.62);
}
.directory-panel {
  width: min(390px, 100%);
  height: 100%;
  overflow-y: auto;
  background: #302842;
  box-shadow: -12px 0 36px rgba(0, 0, 0, 0.35);
}
.directory-panel > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 22px 20px 15px;
  border-bottom: 1px solid rgba(238, 226, 255, 0.14);
}
.directory-panel small {
  color: #c6b5dd;
  font-size: 11px;
}
.directory-panel h2 {
  margin: 4px 0 0;
  color: #fff;
  font-size: 21px;
}
.directory-panel header button {
  background: rgba(255, 255, 255, 0.1);
}
.directory-item {
  display: flex;
  width: 100%;
  min-height: 76px;
  align-items: center;
  gap: 12px;
  padding: 9px 20px;
  border: 0;
  border-bottom: 1px solid rgba(238, 226, 255, 0.1);
  color: #f8f5ff;
  background: transparent;
  text-align: left;
}
.directory-item:hover,
.directory-item.active {
  background: rgba(210, 183, 238, 0.16);
}
.directory-item img {
  width: 72px;
  height: 48px;
  flex: 0 0 auto;
  border-radius: 4px;
  object-fit: cover;
}
.directory-item strong {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
  }
}
</style>
