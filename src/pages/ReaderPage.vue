<script setup lang="ts">
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import {
  computed,
  inject,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import {
  ChevronLeft,
  ChevronRight,
  Columns2,
  EyeOff,
  ListTree,
  LoaderCircle,
  ScrollText,
  X,
  MapPin,
} from "lucide-vue-next";
import { useRouter } from "vue-router";
import { deskInjectionKey } from "../deskContext";
import type { OnlineTextChapter } from "../types";
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
const online = computed(() => desk.readerSource.value === "online");
const book = computed(() => desk.localBook.value);
const chapter = computed(() =>
  online.value ? desk.textChapter.value : desk.localChapter.value,
);
const bookName = computed(() =>
  online.value
    ? desk.onlineBook.value?.novelName || "在线小说"
    : book.value?.name || "本地阅读",
);
const imageDirectory = computed(() => book.value?.imageDirectory || "");
const chapters = computed(() => {
  if (online.value) return desk.textChapters.value;
  return (book.value?.chapterVolumes || []).flatMap((volume) =>
    volume.chapters.map((item) => ({ ...item, volume: volume.volume })),
  );
});
type DirectoryChapter = { id: number; title: string; isVip?: boolean };
type DirectoryVolume = { title: string; chapters: DirectoryChapter[] };
const directoryVolumes = computed<DirectoryVolume[]>(() => {
  if (online.value) {
    return desk.textVolumes.value.map((volume) => ({
      title: volume.title,
      chapters: volume.chapters.map((item) => ({
        id: item.chapId,
        title: item.title,
        isVip: item.isVip,
      })),
    }));
  }
  return (book.value?.chapterVolumes || []).map((volume) => ({
    title: volume.volume,
    chapters: volume.chapters.map((item) => ({
      id: item.id,
      title: item.title,
    })),
  }));
});
const chapterIndex = computed(() =>
  chapters.value.findIndex((item) => item.id === chapter.value?.id),
);
const previousChapter = computed(() => chapters.value[chapterIndex.value - 1]);
const nextChapter = computed(() => chapters.value[chapterIndex.value + 1]);
const controlsVisible = ref(true);
const directoryOpen = ref(false);
const displayMode = ref<DisplayMode>("vertical");
const chapterLoading = ref(false);
const requestedChapterId = ref<number>();
const onlineError = computed(() =>
  online.value ? desk.chapterError.value || desk.directoryError.value : "",
);
const onlineLoading = computed(
  () =>
    online.value && (desk.chapterLoading.value || desk.directoryLoading.value),
);
const onlineNeedsAuth = computed(
  () => online.value && !desk.auth.value.authenticated,
);
const readerStatusTitle = computed(() => {
  if (chapterLoading.value || onlineLoading.value)
    return online.value ? "正在读取在线章节" : "正在打开章节";
  if (onlineError.value) return "加载失败";
  return "从目录选择一章开始阅读";
});

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
function parseLocalChapterParts(rawContent: string): ReaderChapterPart[] {
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

/** 把在线章节的文本与图片占位符拆分为统一阅读块。 */
function parseOnlineChapterParts(
  payload: OnlineTextChapter,
): ReaderChapterPart[] {
  const pattern = /\{\{image:(\d+)\}\}/g;
  const parts: ReaderChapterPart[] = [];
  let lastIndex = 0;
  for (const match of payload.content.matchAll(pattern)) {
    const index = match.index || 0;
    const text = payload.content.slice(lastIndex, index).trim();
    if (text) parts.push({ type: "text", value: text });
    const src = payload.images[Number(match[1])];
    if (src)
      parts.push({
        type: "image",
        alt: `${payload.title} 插图`,
        src,
      });
    lastIndex = index + match[0].length;
  }
  const tail = payload.content.slice(lastIndex).trim();
  if (tail) parts.push({ type: "text", value: tail });
  return parts;
}

const chapterParts = computed<ReaderChapterPart[]>(() => {
  const payload = chapter.value;
  if (!payload) return [];
  if (online.value && desk.textChapter.value)
    return parseOnlineChapterParts(desk.textChapter.value);
  return parseLocalChapterParts(payload.content);
});

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
  if (online.value) {
    const payload = await desk.peekOnlineTextChapter(target.id);
    if (!payload) return undefined;
    return {
      parts: parseOnlineChapterParts(payload),
      title: payload.title,
    };
  }
  const content = await desk.peekLocalChapter(target.id);
  if (!content) return undefined;
  return {
    parts: parseLocalChapterParts(content.content),
    title: content.title,
  };
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
  requestedChapterId.value = chapterId;
  try {
    if (online.value) await desk.openOnlineTextChapter(chapterId);
    else await desk.openLocalChapter(chapterId);
    if (chapter.value?.id !== chapterId) return;
    directoryOpen.value = false;
    await nextTick();
    readerElement.value?.scrollTo({ top: 0, left: 0 });
    if (displayMode.value === "paged") await relayout(options);
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
    if (targetId) await openChapter(targetId);
  } catch {
    // 目录错误会继续展示在页面中。
  }
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

onMounted(async () => {
  if (online.value) {
    if (!desk.onlineBook.value) {
      void router.replace("/remote-book");
      return;
    }
    try {
      await desk.loadDirectory();
      const targetId =
        desk.textChapter.value?.id ||
        desk.pendingChapterId.value ||
        chapters.value[0]?.id;
      if (targetId && desk.textChapter.value?.id !== targetId)
        await openChapter(targetId);
    } catch {
      // 错误已保存在共享状态中并由页面展示。
    }
    return;
  }
  if (!book.value || !chapter.value) {
    void router.replace("/library");
    return;
  }
  desk.navigate("reader");
});

watch(
  () => desk.pendingChapterId.value,
  (chapterId) => {
    if (online.value && chapterId && desk.textChapter.value?.id !== chapterId) {
      void openChapter(chapterId);
    }
  },
);

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

    <div v-if="onlineError" class="reader-error-banner" @click.stop>
      <span>{{ onlineError }}</span>
      <button
        v-if="onlineNeedsAuth"
        type="button"
        @click="desk.openCredentials('official')"
      >
        连接登录
      </button>
      <button type="button" @click="retryOnlineLoad">重试</button>
    </div>

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

  <section v-else class="reader-state">
    <button
      class="reader-state-back"
      type="button"
      title="返回书籍详情"
      aria-label="返回书籍详情"
      @click="goBack"
    >
      <ChevronLeft :size="19" /><span>返回</span>
    </button>
    <LoaderCircle
      v-if="chapterLoading || onlineLoading"
      class="spin"
      :size="30"
    />
    <ListTree v-else :size="30" />
    <strong>{{ readerStatusTitle }}</strong>
    <span v-if="onlineError">{{ onlineError }}</span>
    <span v-else-if="onlineLoading">VIP 正文可能需要识别，请稍候</span>
    <div class="reader-state-actions">
      <button
        v-if="onlineNeedsAuth"
        type="button"
        @click="desk.openCredentials('official')"
      >
        连接登录
      </button>
      <button
        v-if="online && !onlineLoading"
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
        aria-label="小说目录"
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
        <div v-else-if="directoryVolumes.length" class="directory-list">
          <section
            v-for="(volume, volumeIndex) in directoryVolumes"
            :key="volumeIndex"
            class="directory-volume"
          >
            <h3>{{ volume.title }}</h3>
            <button
              v-for="item in volume.chapters"
              :key="item.id"
              class="directory-chapter"
              :class="{ active: item.id === chapter?.id }"
              @click="openChapter(item.id)"
            >
              <strong>{{ item.title }}</strong>
              <span class="directory-row-meta">
                <small v-if="item.isVip" class="vip-badge">VIP</small>
                <MapPin
                  v-if="item.id === chapter?.id"
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
  margin: 50px 0;
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
.directory-volume {
  padding: 8px 0 10px;
}
.directory-volume + .directory-volume {
  border-top: 8px solid #f7f3ea;
}
.directory-volume h3 {
  margin: 0;
  padding: 14px 20px 8px;
  color: #887f70;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.02em;
}
.directory-chapter {
  display: flex;
  width: 100%;
  min-height: 48px;
  align-items: center;
  gap: 10px;
  padding: 0 20px;
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
.directory-chapter strong {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  font-weight: inherit;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.directory-chapter:hover {
  color: #bd6548;
  background: #fff9f1;
}
.directory-chapter.active {
  color: #b85e42;
  background: #fff4ea;
  box-shadow: inset 3px 0 0 #e67c55;
  font-weight: 600;
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
.reader-error-banner {
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
.reader-error-banner span {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.reader-error-banner button,
.reader-state-actions button,
.directory-state button {
  flex: 0 0 auto;
  padding: 6px 10px;
  border: 1px solid rgba(214, 143, 113, 0.56);
  border-radius: 6px;
  color: var(--theme-color-dark);
  background: #fffaf7;
  font-size: 12px;
}
.reader-error-banner button:hover,
.reader-state-actions button:hover,
.directory-state button:hover {
  color: #fff;
  background: var(--theme-color);
}
.reader-state {
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
.reader-state-back {
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
.reader-state-back:hover {
  color: #fff;
  background: var(--theme-color);
}
.reader-state strong {
  font-size: 16px;
}
.reader-state > span {
  max-width: min(560px, 86vw);
  color: var(--muted);
  font-size: 12px;
  line-height: 1.6;
}
.reader-state-actions {
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
  animation: reader-spin 0.9s linear infinite;
}
@keyframes reader-spin {
  to {
    transform: rotate(360deg);
  }
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
