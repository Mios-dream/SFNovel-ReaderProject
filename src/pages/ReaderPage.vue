<script setup lang="ts">
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import {
  ALargeSmall,
  ChevronLeft,
  ChevronRight,
  Columns2,
  ListTree,
  LoaderCircle,
  LockKeyholeOpen,
  ScrollText,
  MapPin,
} from "@lucide/vue";
import { useRouter } from "vue-router";
import {
  usePagedReader,
  type ReaderChapterPart,
  type ReaderStyle,
} from "../reader";
import {
  parseLocalChapterParts,
  parseOnlineChapterParts,
} from "../reader/chapterParts";
import type { DisplayMode } from "../types";
import { asRecord, readPreference, writePreference } from "../utils/preferences";
import DrawerShell from "../components/DrawerShell.vue";
import DirectoryGroupTitle, {
  DIRECTORY_GROUP_HEIGHT,
} from "../components/DirectoryGroupTitle.vue";
import ReaderChrome, {
  type ReaderAction,
} from "../components/ReaderChrome.vue";

import { useAuth, useLibrary, useOnline, useWorkspace } from "../desk";

const auth = useAuth();
const library = useLibrary();
const reading = useOnline();
const workspace = useWorkspace();

type ReaderPanel = "font" | "mode";

const READER_PREFERENCES_KEY = "sf-novel-flow.reader-preferences";
const MIN_FONT_SIZE = 14;
const MAX_FONT_SIZE = 24;
const lineHeightOptions = [
  { value: 1.7, label: "紧凑" },
  { value: 2, label: "标准" },
  { value: 2.3, label: "宽松" },
] as const;

type ReaderPreferences = {
  displayMode: DisplayMode;
  fontSize: number;
  lineHeight: number;
};

const defaultReaderPreferences: ReaderPreferences = {
  displayMode: "paged",
  fontSize: typeof window !== "undefined" && window.innerWidth <= 760 ? 15 : 16,
  lineHeight: 2,
};

function clampFontSize(value: number) {
  return Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, value));
}

/** 校验存量偏好：选项集合变化后旧值要被丢弃，而不是带进渲染。 */
function sanitizeReaderPreferences(raw: unknown): ReaderPreferences {
  const parsed = asRecord(raw) as Partial<ReaderPreferences>;
  return {
    displayMode:
      parsed.displayMode === "vertical" || parsed.displayMode === "paged"
        ? parsed.displayMode
        : defaultReaderPreferences.displayMode,
    fontSize:
      typeof parsed.fontSize === "number" && Number.isFinite(parsed.fontSize)
        ? clampFontSize(Math.round(parsed.fontSize))
        : defaultReaderPreferences.fontSize,
    lineHeight:
      lineHeightOptions.find((option) => option.value === parsed.lineHeight)
        ?.value ?? defaultReaderPreferences.lineHeight,
  };
}

function readReaderPreferences(): ReaderPreferences {
  return readPreference(
    READER_PREFERENCES_KEY,
    sanitizeReaderPreferences,
    () => ({ ...defaultReaderPreferences }),
  );
}

const router = useRouter();
const readerElement = ref<HTMLElement>();
const readerCanvas = ref<HTMLCanvasElement>();
const online = computed(() => workspace.readerSource.value === "online");
const book = computed(() => library.localBook.value);
const chapter = computed(() =>
  online.value ? reading.textChapter.value : library.localChapter.value,
);
const bookName = computed(() =>
  online.value
    ? reading.onlineBook.value?.novelName || "在线小说"
    : book.value?.name || "本地阅读",
);
const imageDirectory = computed(() => book.value?.imageDirectory || "");
const chapters = computed(() => {
  if (online.value) return reading.textChapters.value;
  return (book.value?.chapterVolumes || []).flatMap((volume) =>
    volume.chapters.map((item) => ({ ...item, volume: volume.volume })),
  );
});
type DirectoryChapter = {
  id: number;
  title: string;
  isVip?: boolean;
  isUnlocked?: boolean;
};
type DirectoryVolume = { title: string; chapters: DirectoryChapter[] };
const directoryVolumes = computed<DirectoryVolume[]>(() => {
  if (online.value) {
    return reading.textVolumes.value.map((volume) => ({
      title: volume.title,
      chapters: volume.chapters.map((item) => ({
        id: item.chapId,
        title: item.title,
        isVip: item.isVip,
        isUnlocked: item.accessState === "available",
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
type DirectoryRow =
  | { kind: "volume"; key: string; title: string }
  | {
      kind: "chapter";
      key: string;
      id: number;
      title: string;
      isVip?: boolean;
      isUnlocked?: boolean;
    };
const DIRECTORY_VOLUME_HEIGHT = DIRECTORY_GROUP_HEIGHT;
const DIRECTORY_CHAPTER_HEIGHT = 48;
const DIRECTORY_OVERSCAN = 6;
const directoryRows = computed<DirectoryRow[]>(() =>
  directoryVolumes.value.flatMap((volume, volumeIndex) => [
    {
      kind: "volume" as const,
      key: `volume-${volumeIndex}`,
      title: volume.title,
    },
    ...volume.chapters.map((item) => ({
      kind: "chapter" as const,
      key: `chapter-${volumeIndex}-${item.id}`,
      id: item.id,
      title: item.title,
      isVip: item.isVip,
      isUnlocked: item.isUnlocked,
    })),
  ]),
);
const directoryOffsets = computed(() => {
  const offsets: number[] = [];
  let offset = 0;
  for (const row of directoryRows.value) {
    offsets.push(offset);
    offset +=
      row.kind === "volume"
        ? DIRECTORY_VOLUME_HEIGHT
        : DIRECTORY_CHAPTER_HEIGHT;
  }
  offsets.push(offset);
  return offsets;
});
const directoryScrollTop = ref(0);
const directoryViewportHeight = ref(0);
const directoryWindow = computed(() => {
  const rows = directoryRows.value;
  const offsets = directoryOffsets.value;
  const top = directoryScrollTop.value;
  let start = 0;
  while (start < rows.length - 1 && offsets[start + 1] <= top) start++;
  let end = start;
  while (
    end < rows.length &&
    offsets[end] < top + directoryViewportHeight.value
  )
    end++;
  const from = Math.max(0, start - DIRECTORY_OVERSCAN);
  const to = Math.min(rows.length, end + DIRECTORY_OVERSCAN);
  return {
    rows: rows.slice(from, to),
    top: offsets[from],
    bottom: offsets[rows.length] - offsets[to],
  };
});
const chapterIndex = computed(() =>
  chapters.value.findIndex((item) => item.id === chapter.value?.id),
);
const previousChapter = computed(() => chapters.value[chapterIndex.value - 1]);
const nextChapter = computed(() => chapters.value[chapterIndex.value + 1]);
const controlsVisible = ref(true);
const directoryOpen = ref(false);
const directoryList = ref<HTMLElement>();
const initialPreferences = readReaderPreferences();
const displayMode = ref<DisplayMode>(initialPreferences.displayMode);
const fontSize = ref(initialPreferences.fontSize);
const lineHeight = ref(initialPreferences.lineHeight);
const activePanel = ref<ReaderPanel | null>(null);
const verticalProgress = ref(0);
const chapterLoading = ref(false);
const requestedChapterId = ref<number>();
const onlineError = computed(() =>
  online.value ? reading.chapterError.value || reading.directoryError.value : "",
);
const onlineLoading = computed(
  () =>
    online.value && (reading.chapterLoading.value || reading.directoryLoading.value),
);
const onlineNeedsAuth = computed(
  () => online.value && !auth.auth.value.authenticated,
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
const chapterParts = computed<ReaderChapterPart[]>(() => {
  if (!chapter.value) return [];
  if (online.value && reading.textChapter.value)
    return parseOnlineChapterParts(reading.textChapter.value);
  return parseLocalChapterParts(chapter.value.content, chapterImageSource);
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
    fontSize: fontSize.value,
    lineHeight: lineHeight.value,
    color: root.getPropertyValue("--ink").trim() || "#553b37",
    background: root.getPropertyValue("--background-color").trim() || "#fcede6",
    padding: mobile
      ? {
          top: 66 + readSafeArea("top"),
          right: 12,
          bottom: 32 + readSafeArea("bottom"),
          left: 12,
        }
      : { top: 76, right: 48, bottom: 32, left: 48 },
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
    else toggleControls();
  },
});

const canGoPrevious = computed(() =>
  displayMode.value === "vertical"
    ? Boolean(previousChapter.value)
    : pageIndex.value > 0 || Boolean(previousChapter.value),
);

const canGoNext = computed(() =>
  displayMode.value === "vertical"
    ? Boolean(nextChapter.value)
    : pageIndex.value < pageCount.value - 1 || Boolean(nextChapter.value),
);

const chapterProgress = computed(() =>
  displayMode.value === "paged"
    ? pageCount.value > 0
      ? (pageIndex.value + 1) / pageCount.value
      : 0
    : verticalProgress.value,
);

const progressPercent = computed(() =>
  Math.min(100, Math.max(0, Math.round(chapterProgress.value * 100))),
);

/** 预取相邻章节正文（不改变当前阅读状态），供翻页模式预渲染边界页。 */
async function prepareNeighbor(direction: "previous" | "next") {
  const target =
    direction === "previous" ? previousChapter.value : nextChapter.value;
  if (!target) return undefined;
  if (online.value) {
    const payload = await reading.peekOnlineTextChapter(target.id);
    if (!payload) return undefined;
    return {
      parts: parseOnlineChapterParts(payload),
      title: payload.title,
    };
  }
  const content = await library.peekLocalChapter(target.id);
  if (!content) return undefined;
  return {
    parts: parseLocalChapterParts(content.content, chapterImageSource),
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
    if (online.value) await reading.openOnlineTextChapter(chapterId);
    else await library.openLocalChapter(chapterId);
    if (chapter.value?.id !== chapterId) return;
    directoryOpen.value = false;
    await nextTick();
    readerElement.value?.scrollTo({ top: 0, left: 0 });
    updateScrollProgress();
    if (displayMode.value === "paged") await relayout(options);
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
    if (targetId) await openChapter(targetId);
  } catch {
    // 目录错误会继续展示在页面中。
  }
}

function saveReaderPreferences() {
  writePreference(READER_PREFERENCES_KEY, {
    displayMode: displayMode.value,
    fontSize: fontSize.value,
    lineHeight: lineHeight.value,
  } satisfies ReaderPreferences);
}

function changeFontSize(delta: number) {
  const next = clampFontSize(fontSize.value + delta);
  if (next === fontSize.value) return;
  fontSize.value = next;
  saveReaderPreferences();
  void nextTick(() => {
    if (displayMode.value === "paged") void relayout();
    else updateScrollProgress();
  });
}

function setLineHeight(value: number) {
  if (lineHeight.value === value) return;
  lineHeight.value = value;
  saveReaderPreferences();
  void nextTick(() => {
    if (displayMode.value === "paged") void relayout();
    else updateScrollProgress();
  });
}

function toggleReaderPanel(panel: ReaderPanel) {
  activePanel.value = activePanel.value === panel ? null : panel;
}

/** 底部控制栏的三个入口；顺序与漫画阅读器一致，避免两页位置漂移。 */
const readerActions = computed<ReaderAction[]>(() => [
  { key: "directory", label: "目录" },
  { key: "font", label: "字体", active: activePanel.value === "font" },
  { key: "mode", label: "阅读方式", active: activePanel.value === "mode" },
]);

function handleReaderAction(key: string) {
  if (key === "directory") showDirectory();
  else if (key === "font" || key === "mode") toggleReaderPanel(key);
}

function toggleControls() {
  controlsVisible.value = !controlsVisible.value;
  if (!controlsVisible.value) activePanel.value = null;
}

function updateScrollProgress() {
  if (displayMode.value !== "vertical") return;
  const element = readerElement.value;
  if (!element) {
    verticalProgress.value = 0;
    return;
  }
  const scrollRange = element.scrollHeight - element.clientHeight;
  verticalProgress.value =
    scrollRange <= 0
      ? 1
      : Math.min(1, Math.max(0, element.scrollTop / scrollRange));
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
  saveReaderPreferences();
  void nextTick(() => {
    readerElement.value?.scrollTo({ top: 0, left: 0 });
    if (mode === "paged") void relayout();
    else updateScrollProgress();
  });
}

function handleReaderClick(event: MouseEvent) {
  // 翻页模式下的点击（翻页与菜单显隐）由翻页引擎统一处理。
  if (displayMode.value === "paged") return;
  const rect = readerElement.value?.getBoundingClientRect();
  if (!rect) return;
  const localY = event.clientY - rect.top;
  if (localY < rect.height * 0.25 || localY > rect.height * 0.75) return;
  toggleControls();
}

function showDirectory() {
  controlsVisible.value = false;
  activePanel.value = null;
  directoryOpen.value = true;
}

async function revealCurrentChapter() {
  if (!directoryOpen.value || reading.directoryLoading.value) return;
  await nextTick();
  const list = directoryList.value;
  if (!list) return;
  directoryViewportHeight.value = list.clientHeight;
  const index = directoryRows.value.findIndex(
    (row) => row.kind === "chapter" && row.id === chapter.value?.id,
  );
  if (index >= 0) {
    list.scrollTop = Math.max(
      0,
      directoryOffsets.value[index] -
        (list.clientHeight - DIRECTORY_CHAPTER_HEIGHT) / 2,
    );
  }
  directoryScrollTop.value = list.scrollTop;
}

function setDirectoryBody(element: HTMLElement | null) {
  directoryList.value = element ?? undefined;
}

function handleDirectoryScroll() {
  const list = directoryList.value;
  if (!list) return;
  directoryScrollTop.value = list.scrollTop;
  directoryViewportHeight.value = list.clientHeight;
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
        reading.textChapter.value?.id ||
        reading.pendingChapterId.value ||
        chapters.value[0]?.id;
      if (targetId && reading.textChapter.value?.id !== targetId)
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
  workspace.navigate("reader");
  await nextTick();
  updateScrollProgress();
});

watch(
  () => reading.pendingChapterId.value,
  (chapterId) => {
    if (online.value && chapterId && reading.textChapter.value?.id !== chapterId) {
      void openChapter(chapterId);
    }
  },
);

watch(
  [directoryOpen, () => reading.directoryLoading.value, () => chapter.value?.id],
  () => void revealCurrentChapter(),
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
    @scroll.passive="updateScrollProgress"
  >
    <div v-if="onlineError" class="reader-error-banner" @click.stop>
      <span>{{ onlineError }}</span>
      <button
        v-if="onlineNeedsAuth"
        type="button"
        @click="auth.openCredentials('official')"
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
      <div
        v-if="displayMode === 'vertical'"
        class="chapter-content"
        :style="{ fontSize: `${fontSize}px`, lineHeight }"
      >
        <h1 class="chapter-title">{{ chapter.title }}</h1>
        <template v-for="(part, index) in chapterParts" :key="index">
          <img
            v-if="part.type === 'image'"
            class="chapter-image"
            :src="part.src"
            :alt="part.alt"
            @load="updateScrollProgress"
          />
          <p v-else class="chapter-text">{{ part.value }}</p>
        </template>
      </div>
      <div v-else class="page-flip-frame">
        <canvas ref="readerCanvas" class="page-flip-canvas" />
        <header
          v-show="!controlsVisible"
          class="page-flip-chrome page-flip-topbar"
          @click.stop
        >
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
          v-show="!controlsVisible"
          class="page-flip-chrome page-flip-bottom-meta"
          aria-label="书名和页数"
        >
          <span>{{ bookName }}</span>
          <span>{{ pageIndex + 1 }} / {{ Math.max(pageCount, 1) }}</span>
        </div>
      </div>
    </main>

    <ReaderChrome
      :visible="controlsVisible"
      :subtitle="chapter.volume"
      :title="chapter.title"
      :actions="readerActions"
      @back="goBack"
      @action="handleReaderAction"
    >
      <template #panel>
        <Transition name="reader-swap" mode="out-in">
          <section
            v-if="activePanel === 'font'"
            key="font"
            class="reader-panel"
            aria-label="字体设置"
          >
            <div class="reader-setting-row">
              <span class="reader-setting-label">字号</span>
              <div class="reader-stepper">
                <button
                  type="button"
                  title="减小字号"
                  aria-label="减小字号"
                  :disabled="fontSize <= MIN_FONT_SIZE"
                  @click="changeFontSize(-1)"
                >
                  −
                </button>
                <span class="reader-stepper-value">{{ fontSize }}</span>
                <button
                  type="button"
                  title="增大字号"
                  aria-label="增大字号"
                  :disabled="fontSize >= MAX_FONT_SIZE"
                  @click="changeFontSize(1)"
                >
                  +
                </button>
              </div>
            </div>
            <div class="reader-setting-row">
              <span class="reader-setting-label">行距</span>
              <div class="reader-segment">
                <button
                  v-for="option in lineHeightOptions"
                  :key="option.value"
                  type="button"
                  :class="{ active: lineHeight === option.value }"
                  :aria-pressed="lineHeight === option.value"
                  @click="setLineHeight(option.value)"
                >
                  {{ option.label }}
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
      <template #font><ALargeSmall :size="20" /></template>
      <template #mode>
        <ScrollText v-if="displayMode === 'vertical'" :size="20" />
        <Columns2 v-else :size="20" />
      </template>
    </ReaderChrome>
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
        @click="auth.openCredentials('official')"
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

  <DrawerShell
    :open="directoryOpen"
    align="start"
    title="章节目录"
    :subtitle="bookName"
    :body-ref="setDirectoryBody"
    @close="directoryOpen = false"
    @scroll="handleDirectoryScroll"
  >
    <div v-if="reading.directoryLoading.value" class="drawer-state">
      <LoaderCircle class="spin" :size="24" />正在读取目录
    </div>
    <div v-else-if="reading.directoryError.value" class="drawer-state error">
      <span>{{ reading.directoryError.value }}</span>
      <button type="button" @click="retryOnlineLoad">重试</button>
    </div>
    <template v-else-if="directoryRows.length">
      <div
        :style="{ height: `${directoryWindow.top}px` }"
        aria-hidden="true"
      ></div>
      <template v-for="row in directoryWindow.rows" :key="row.key">
        <DirectoryGroupTitle
          v-if="row.kind === 'volume'"
          :label="row.title"
        />
        <button
          v-else
          class="directory-chapter"
          :class="{ active: row.id === chapter?.id }"
          @click="openChapter(row.id)"
        >
          <strong>{{ row.title }}</strong>
          <span class="directory-row-meta">
            <small
              v-if="row.isVip"
              class="vip-badge"
              :class="{ unlocked: row.isUnlocked === true }"
              ><LockKeyholeOpen
                v-if="row.isUnlocked === true"
                :size="12"
                aria-label="已解锁"
              />VIP</small
            >
            <MapPin
              v-if="row.id === chapter?.id"
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
  margin: 0;
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
.directory-chapter {
  display: flex;
  width: 100%;
  height: 48px;
  align-items: center;
  gap: 10px;
  padding: 0 18px;
  border: 0;
  border-top: 1px solid rgba(202, 174, 159, 0.16);
  color: var(--ink);
  background: transparent;
  font: inherit;
  font-size: 13px;
  font-weight: 500;
  text-align: left;
  transition:
    color 150ms ease,
    background 150ms ease;
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
  color: var(--theme-color-dark);
  background: rgba(246, 195, 160, 0.2);
}
.directory-chapter.active {
  color: #a85a39;
  background: rgba(226, 148, 100, 0.16);
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
  color: var(--theme-color-dark);
}
.vip-badge {
  display: inline-flex;
  min-height: 20px;
  flex: 0 0 auto;
  align-items: center;
  gap: 3px;
  padding: 0 6px;
  border-radius: 999px;
  color: #a85a39;
  background: rgba(226, 148, 100, 0.16);
  font-size: 12px;
  font-weight: 500;
  white-space: nowrap;
}
.vip-badge.unlocked {
  color: #4f8068;
  background: #e7f1ea;
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
.reader-state-actions button {
  flex: 0 0 auto;
  padding: 6px 10px;
  border: 1px solid rgba(214, 143, 113, 0.56);
  border-radius: 6px;
  color: var(--theme-color-dark);
  background: #fffaf7;
  font-size: 12px;
}
.reader-error-banner button:hover,
.reader-state-actions button:hover {
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
