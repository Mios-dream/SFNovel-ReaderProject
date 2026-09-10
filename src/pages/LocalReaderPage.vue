<script setup lang="ts">
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
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
import {
  usePagedReader,
  type ReaderChapterPart,
  type ReaderStyle,
} from "../reader";

function requireDesk() {
  const desk = inject(deskInjectionKey);
  if (!desk) throw new Error("Desk context is unavailable");
  return desk;
}

type DisplayMode = "vertical" | "paged";

const desk = requireDesk();
const router = useRouter();
const readerElement = ref<HTMLElement>();
const stageElement = ref<HTMLElement>();
const readerCanvas = ref<HTMLCanvasElement>();
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

function chapterImageSource(relativePath: string) {
  const fileName = relativePath.slice("imgs/".length).replace(/\\/g, "/");
  const separator = imageDirectory.value.includes("\\") ? "\\" : "/";
  const imagePath = `${imageDirectory.value.replace(/[\\/]+$/, "")}${separator}${fileName.replace(/\//g, separator)}`;
  return isTauri() ? convertFileSrc(imagePath) : imagePath;
}

/**
 * 把章节 markdown 内容拆分为文本段与插图，两种显示模式共用。
 * 同时用于当前章节与预渲染的相邻章节。
 */
function parseChapterParts(rawContent: string): ReaderChapterPart[] {
  const content = rawContent.replace(/^##\s+.+\r?\n+/, "");
  const pattern = /!\[([^\]]*)\]\((imgs[\\/][^)]+)\)/g;
  const parts: ReaderChapterPart[] = [];
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
}

const chapterParts = computed<ReaderChapterPart[]>(() =>
  parseChapterParts(chapter.value?.content || ""),
);

/**
 * 读取安全区尺寸。canvas 无法直接使用 CSS 的 env()，
 * 故通过一个临时探针元素间接测量；仅在重新分页时调用，开销可忽略。
 */
function readSafeArea(side: "top" | "bottom"): number {
  const probe = document.createElement("div");
  probe.style.cssText = `position:fixed;visibility:hidden;pointer-events:none;padding-${side}:env(safe-area-inset-${side},0px);`;
  document.body.appendChild(probe);
  const value =
    Number.parseFloat(
      getComputedStyle(probe).getPropertyValue(`padding-${side}`),
    ) || 0;
  probe.remove();
  return value;
}

/** 计算画布分页样式，与纵向滚动模式的 CSS 观感保持一致。 */
function readerStyle(): ReaderStyle {
  const root = getComputedStyle(document.documentElement);
  const mobile = window.innerWidth <= 760;
  return {
    fontFamily: '"Microsoft YaHei", sans-serif',
    fontSize: mobile ? 15 : 16,
    lineHeight: 2,
    color: root.getPropertyValue("--ink").trim() || "#553b37",
    background: root.getPropertyValue("--background-color").trim() || "#fcede6",
    padding: mobile
      ? {
          top: 66 + readSafeArea("top"),
          right: 12,
          bottom: 24 + readSafeArea("bottom"),
          left: 12,
        }
      : { top: 76, right: 48, bottom: 90, left: 48 },
  };
}

const { pageIndex, pageCount, relayout, goNext, goPrev } = usePagedReader({
  canvas: readerCanvas,
  parts: () => chapterParts.value,
  title: () => chapter.value?.title || "",
  style: readerStyle,
  prepareNeighbor,
  onReachEnd: () => void openChapter(nextChapter.value?.id, { adopt: "next" }),
  onReachStart: () =>
    void openChapter(previousChapter.value?.id, {
      atEnd: true,
      adopt: "previous",
    }),
  onTap: (zone) => {
    if (zone === "prev") previous();
    else if (zone === "next") next();
    else controlsVisible.value = !controlsVisible.value;
  },
});

/** 预取相邻章节正文（不改变当前阅读状态），供翻页模式预渲染边界页。 */
async function prepareNeighbor(direction: "previous" | "next") {
  const target =
    direction === "previous" ? previousChapter.value : nextChapter.value;
  if (!target) return undefined;
  const content = await desk.peekLocalChapter(target.id);
  if (!content) return undefined;
  return { parts: parseChapterParts(content.content), title: content.title };
}

/**
 * 打开章节。
 * @param options.atEnd 定位到末页（从下一章回退时）。
 * @param options.adopt 沿用边界滑动时已显示的相邻章节位图，避免闪动。
 */
async function openChapter(
  chapterId?: number,
  options: { atEnd?: boolean; adopt?: "previous" | "next" } = {},
) {
  if (!chapterId || chapterLoading.value) return;
  chapterLoading.value = true;
  try {
    await desk.openLocalChapter(chapterId);
    directoryOpen.value = false;
    await nextTick();
    readerElement.value?.scrollTo({ top: 0, left: 0 });
    if (displayMode.value === "paged") await relayout(options);
  } finally {
    chapterLoading.value = false;
  }
}

function goBack() {
  desk.navigate("libraryDetail");
  void router.push(`/library/${encodeURIComponent(bookName.value)}`);
}

function previous() {
  if (displayMode.value === "paged") {
    goPrev();
    return;
  }
  void openChapter(previousChapter.value?.id);
}

function next() {
  if (displayMode.value === "paged") {
    goNext();
    return;
  }
  void openChapter(nextChapter.value?.id);
}

function setDisplayMode(mode: DisplayMode) {
  if (displayMode.value === mode) return;
  displayMode.value = mode;
  controlsVisible.value = mode === "vertical";
  void nextTick(() => {
    readerElement.value?.scrollTo({ top: 0, left: 0 });
    if (mode === "paged") void relayout();
  });
}

function handleReaderClick(event: MouseEvent) {
  // 翻页模式下的点击（翻页与菜单显隐）由翻页引擎统一处理。
  if (displayMode.value === "paged") return;
  const rect = readerElement.value?.getBoundingClientRect();
  if (!rect) return;
  const localY = event.clientY - rect.top;
  if (localY < rect.height * 0.25 || localY > rect.height * 0.75) return;
  controlsVisible.value = !controlsVisible.value;
}

function showDirectory() {
  directoryOpen.value = true;
}

onMounted(() => {
  if (!book.value || !chapter.value) void router.replace("/library");
  else desk.navigate("reader");
});

onBeforeUnmount(() => {
  directoryOpen.value = false;
});
</script>

<template>
  <article
    v-if="chapter"
    ref="readerElement"
    class="reader"
    :class="{ 'paged-reader': displayMode === 'paged' }"
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
      <div v-else class="page-flip-frame">
        <canvas ref="readerCanvas" class="page-flip-canvas" />
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
        <div
          class="page-flip-chrome page-flip-bottom-meta"
          aria-label="书名和页数"
        >
          <span>{{ bookName }}</span>
          <span>{{ pageIndex + 1 }} / {{ Math.max(pageCount, 1) }}</span>
        </div>
      </div>
    </main>

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
            : pageIndex === 0 && !previousChapter
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
            : pageIndex >= pageCount - 1 && !nextChapter
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
  touch-action: none;
}
.page-flip-canvas {
  display: block;
  width: 100%;
  max-width: 820px;
  height: 100%;
}
.page-flip-chrome {
  position: absolute;
  z-index: 20;
  color: var(--theme-color-dark);
  /* 仅按钮本身开启命中，容器不遮挡画布手势。 */
  pointer-events: none;
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
  pointer-events: auto;
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
