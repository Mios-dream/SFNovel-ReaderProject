<script setup lang="ts">
import {
  computed,
  onMounted,
  ref,
  watch,
  type ComponentPublicInstance,
} from "vue";
import type { ExportFormat, LocalWorkMetadata, MediaType } from "../types";
import { useRoute, useRouter } from "vue-router";
import {
  BookOpen,
  ChevronLeft,
  ChevronRight,
  Download,
  ExternalLink,
  FileDown,
  Headphones,
  Image as ImageIcon,
  Images,
  LibraryBig,
  ListTree,
  Play,
  Heart,
  Star,
  Ticket,
  X,
} from "@lucide/vue";
import ExportModal from "../components/ExportModal.vue";
import DrawerShell from "../components/DrawerShell.vue";
import DirectoryGroupTitle, {
  DIRECTORY_GROUP_HEIGHT,
} from "../components/DirectoryGroupTitle.vue";
import ModalShell from "../components/ModalShell.vue";
import { useVirtualRows } from "../composables/useVirtualRows";
import {
  formatMetric,
  formatSourceDate,
  scoreToneClass,
} from "../utils/format";

import { useLibrary, useWorkspace } from "../desk";

const library = useLibrary();
const workspace = useWorkspace();

const route = useRoute();
const router = useRouter();
const book = computed(() => library.localBook.value);
const exporting = library.exportingFormat;
const mediaTypes: MediaType[] = ["novel", "audio", "comic"];

const textChapterCount = computed(() =>
  (book.value?.chapterVolumes ?? []).reduce(
    (count, volume) => count + volume.chapters.length,
    0,
  ),
);
const audioChapterCount = computed(() => book.value?.audioTracks.length ?? 0);
const comicChapterCount = computed(() => book.value?.comicChapters.length ?? 0);

/** 三类内容的章节数，供切换按钮、导出可用性与初始选中统一取值。 */
const mediaChapterCounts = computed<Record<MediaType, number>>(() => ({
  novel: textChapterCount.value,
  audio: audioChapterCount.value,
  comic: comicChapterCount.value,
}));

/**
 * 按 文字 → 有声 → 漫画 的顺序取第一个有内容的类型。
 *
 * # 参数
 * - `fallback`：三种内容都为空时返回的类型。详情页兜底漫画（空目录），
 *   侧栏兜底小说，故由调用方决定。
 */
function firstMediaWithContent(fallback: MediaType): MediaType {
  return (
    mediaTypes.find((media) => mediaChapterCounts.value[media] > 0) ?? fallback
  );
}

const selectedMediaType = ref<MediaType>(firstMediaWithContent("comic"));
const emptyWork: LocalWorkMetadata = {
  id: 0,
  title: "未命名作品",
  author: "未知作者",
  description: "",
  tags: [],
};
const currentWork = computed<LocalWorkMetadata>(
  () => book.value?.[selectedMediaType.value] ?? emptyWork,
);
const firstTextChapterId = computed(
  () => book.value?.chapterVolumes[0]?.chapters[0]?.id,
);
const chapterMode = ref<MediaType>(firstMediaWithContent("comic"));
const desktopContentTab = ref<MediaType>(firstMediaWithContent("novel"));
const desktopDirectoryCount = computed(
  () => mediaChapterCounts.value[desktopContentTab.value],
);
type LocalDirectoryRow =
  | { kind: "gap"; key: string; height: number }
  | { kind: "volume"; key: string; title: string; height: number }
  | { kind: "text"; key: string; id: number; title: string; height: number }
  | {
      kind: "audio";
      key: string;
      index: number;
      title: string;
      height: number;
    }
  | {
      kind: "comic";
      key: string;
      id: number;
      title: string;
      cover?: string;
      height: number;
    };
/**
 * 虚拟列表的行高，桌面与移动两套视觉尺寸。
 *
 * 行高必须与实际 CSS 一致，否则滚动位置会跳；key 前缀分开是为了让两个
 * 列表各自的复用状态互不干扰。
 */
type DirectoryRowMetrics = {
  prefix: string;
  gap: number;
  volume: number;
  text: number;
  audio: number;
  comic: number;
};

const DESKTOP_ROW_METRICS: DirectoryRowMetrics = {
  prefix: "desktop",
  gap: 8,
  volume: 40,
  text: 40,
  audio: 40,
  comic: 62,
};

const MOBILE_ROW_METRICS: DirectoryRowMetrics = {
  prefix: "mobile",
  gap: 9,
  volume: DIRECTORY_GROUP_HEIGHT,
  text: 48,
  audio: 48,
  comic: 66,
};

/**
 * 把作品展开成虚拟列表行。
 *
 * 桌面侧栏与移动抽屉用的是同一份结构，原先各写一遍，行高与 key 前缀散落
 * 在两处，容易只改一侧。
 */
function buildDirectoryRows(
  mode: MediaType,
  metrics: DirectoryRowMetrics,
): LocalDirectoryRow[] {
  const localBook = book.value;
  if (!localBook) return [];

  const rows: LocalDirectoryRow[] = [];
  const { prefix } = metrics;

  if (mode === "novel") {
    localBook.chapterVolumes.forEach((volume, volumeIndex) => {
      if (volumeIndex > 0)
        rows.push({
          kind: "gap",
          key: `${prefix}-gap-${volumeIndex}`,
          height: metrics.gap,
        });
      rows.push({
        kind: "volume",
        key: `${prefix}-volume-${volumeIndex}`,
        title: volume.volume,
        height: metrics.volume,
      });
      for (const chapter of volume.chapters)
        rows.push({
          kind: "text",
          key: `${prefix}-text-${volumeIndex}-${chapter.id}`,
          id: chapter.id,
          title: chapter.title,
          height: metrics.text,
        });
    });
    return rows;
  }

  if (mode === "audio") {
    localBook.audioTracks.forEach((track, index) =>
      rows.push({
        kind: "audio",
        key: `${prefix}-audio-${track.href}`,
        index,
        title: track.title,
        height: metrics.audio,
      }),
    );
    return rows;
  }

  localBook.comicChapters.forEach((chapter) =>
    rows.push({
      kind: "comic",
      key: `${prefix}-comic-${chapter.id}`,
      id: chapter.id,
      title: chapter.title,
      cover: chapter.cover,
      height: metrics.comic,
    }),
  );
  return rows;
}

const desktopDirectoryRows = computed(() =>
  buildDirectoryRows(desktopContentTab.value, DESKTOP_ROW_METRICS),
);
const mobileDirectoryRows = computed(() =>
  buildDirectoryRows(chapterMode.value, MOBILE_ROW_METRICS),
);
const {
  container: desktopDirectoryList,
  windowed: desktopDirectoryWindow,
  handleScroll: handleDesktopDirectoryScroll,
  reset: resetDesktopDirectory,
} = useVirtualRows(() => desktopDirectoryRows.value);
const {
  container: mobileDirectoryList,
  windowed: mobileDirectoryWindow,
  handleScroll: handleMobileDirectoryScroll,
  reset: resetMobileDirectory,
} = useVirtualRows(() => mobileDirectoryRows.value);
watch([desktopContentTab, chapterMode], () => {
  resetDesktopDirectory();
  resetMobileDirectory();
});
function setDesktopDirectoryElement(
  element: Element | ComponentPublicInstance | null,
) {
  desktopDirectoryList.value =
    element instanceof HTMLElement ? element : undefined;
}
function setMobileDirectoryElement(
  element: Element | ComponentPublicInstance | null,
) {
  mobileDirectoryList.value =
    element instanceof HTMLElement ? element : undefined;
}
const mobileSwitchMediaTypes = computed(() =>
  mediaTypes.filter((media) => media !== selectedMediaType.value),
);
const directoryOpen = ref(false);
const exportModalOpen = ref(false);
const descriptionExpanded = ref(false);
const descriptionModalOpen = ref(false);
const desktopDirectoryElement = ref<HTMLElement>();
const primaryActionLabel = computed(() =>
  selectedMediaType.value === "comic"
    ? "阅读漫画"
    : selectedMediaType.value === "audio"
      ? "播放有声"
      : textChapterCount.value
        ? "开始阅读"
        : "播放有声",
);
const localStatusLabel = computed(() => {
  if (currentWork.value.isFinished == null) return "";
  return currentWork.value.isFinished ? "已完结" : "连载中";
});
const desktopBookFacts = computed(() => {
  const currentBook = currentWork.value;

  const facts: Array<{ label: string; value: string; title?: string }> = [];
  const updatedAt = currentBook.latestChapterTime || currentBook.lastUpdateTime;
  if (updatedAt) {
    facts.push({ label: "最近更新", value: formatSourceDate(updatedAt) });
  }
  if (currentBook.viewCount != null) {
    facts.push({ label: "热度", value: formatMetric(currentBook.viewCount) });
  }
  if (currentBook.pointCount != null) {
    facts.push({ label: "人气", value: formatMetric(currentBook.pointCount) });
  }
  if (currentBook.favoriteCount != null) {
    facts.push({
      label: "收藏",
      value: formatMetric(currentBook.favoriteCount),
    });
  }
  return facts;
});

/**
 * 三种本地内容的展示口径。
 *
 * 原先 mediaTitle / mediaDescription / canExportCurrentMedia / exportActionLabel
 * 各写一遍 `media === "xxx"` 的分支，新增一种内容要改四处、漏一处只会静默显示错文案。
 */
const MEDIA_KINDS: Record<
  MediaType,
  { title: string; exportAction: string; countUnit: string; countEmpty: string }
> = {
  novel: {
    title: "小说",
    exportAction: "导出小说",
    countUnit: "个本地章节",
    countEmpty: "暂无本地章节",
  },
  audio: {
    title: "有声",
    exportAction: "导出有声",
    countUnit: "条本地音轨",
    countEmpty: "暂无本地音轨",
  },
  comic: {
    title: "漫画",
    exportAction: "导出漫画",
    countUnit: "个本地章节",
    countEmpty: "暂无本地漫画章节",
  },
};

/**
 * 打开作品时默认展示哪类内容。
 *
 * 与 `firstMediaWithContent` 的区别：这里优先看「有没有该类型的元数据」，
 * 因为下载只有封面与简介、尚无章节的作品也应该先展示它自己的类型。
 */
function preferredMediaType(): MediaType {
  if (book.value?.novel) return "novel";
  if (book.value?.audio) return "audio";
  if (book.value?.comic) return "comic";
  return firstMediaWithContent("comic");
}

/** 该类型是否已下载到本地，决定内容切换按钮是否可点。 */
function hasCachedMedia(media: MediaType) {
  return Boolean(book.value?.[media]);
}

function mediaTitle(media: MediaType) {
  return MEDIA_KINDS[media].title;
}

function mediaDescription(media: MediaType) {
  const kind = MEDIA_KINDS[media];
  const count = mediaChapterCounts.value[media];
  return count ? `${count} ${kind.countUnit}` : kind.countEmpty;
}

const canExportCurrentMedia = computed(
  () => mediaChapterCounts.value[selectedMediaType.value] > 0,
);

function exportCurrentMedia() {
  if (canExportCurrentMedia.value) exportModalOpen.value = true;
}

function exportFormat(format: ExportFormat) {
  exportModalOpen.value = false;
  library.exportBook(format);
}

const exportActionLabel = computed(
  () => MEDIA_KINDS[selectedMediaType.value].exportAction,
);

function setMediaType(media: MediaType) {
  selectedMediaType.value = media;
  desktopContentTab.value = media;
  descriptionExpanded.value = false;
  descriptionModalOpen.value = false;
}

async function loadRouteBook() {
  const name = route.params.name;
  if (typeof name !== "string" || !name) {
    await router.replace("/library");
    return;
  }
  if (library.localBook.value?.name !== name) {
    const loadedBook = await library.loadLocalBook(name);
    if (!loadedBook) {
      await router.replace("/library");
      return;
    }
  }
  const media = preferredMediaType();
  chapterMode.value = media;
  setMediaType(media);
}

function goBack() {
  workspace.back();
}

async function readChapter(chapterId: number) {
  directoryOpen.value = false;
  await library.openLocalChapter(chapterId);
  if (library.localChapter.value) {
    await workspace.navigate("reader");
  }
}

async function playAudio(trackIndex: number) {
  directoryOpen.value = false;
  library.openLocalAudioPlayer(trackIndex);
  if (library.localBook.value?.audioTracks.length) {
    await workspace.navigate("audioPlayer");
  }
}

async function readComicChapter(chapterId: number) {
  directoryOpen.value = false;
  await library.openLocalComicChapter(chapterId);
  if (library.localComicChapter.value) {
    await workspace.navigate("comicReader");
  }
}

function startReading() {
  if (selectedMediaType.value === "comic" && comicChapterCount.value) {
    void readComicChapter(book.value!.comicChapters[0].id);
    return;
  }
  if (selectedMediaType.value === "audio" && audioChapterCount.value) {
    void playAudio(0);
    return;
  }
  if (firstTextChapterId.value) {
    void readChapter(firstTextChapterId.value);
    return;
  }
  if (audioChapterCount.value) void playAudio(0);
  else if (comicChapterCount.value)
    void readComicChapter(book.value!.comicChapters[0].id);
}

/** 某类内容已下载的章节数，用于判断能否切换或打开目录。 */
function chapterCountOf(media: MediaType) {
  return mediaChapterCounts.value[media];
}

/**
 * 在已有内容的类型里挑一个：优先 `preferred`，否则按文字 → 有声 → 漫画回退。
 *
 * # 返回
 * 有内容的模式；三种内容都为空时返回 `undefined`，由调用方决定如何处理——
 * 打开目录时退到漫画（空目录），切换侧栏时保持当前模式。
 */
function resolveAvailableChapterMode(
  preferred?: MediaType,
): MediaType | undefined {
  if (preferred && chapterCountOf(preferred) > 0) return preferred;
  for (const mode of ["novel", "audio", "comic"] as const) {
    if (chapterCountOf(mode) > 0) return mode;
  }
  return undefined;
}

function openDirectory(mode?: MediaType) {
  chapterMode.value =
    resolveAvailableChapterMode(mode ?? selectedMediaType.value) ?? "comic";
  directoryOpen.value = true;
}

function focusDesktopDirectory(mode?: MediaType) {
  const resolved = resolveAvailableChapterMode(mode ?? selectedMediaType.value);
  if (!resolved) {
    setMediaType("novel");
    return;
  }
  setMediaType(resolved);
  chapterMode.value = resolved;
  desktopDirectoryElement.value?.scrollIntoView({
    behavior: "smooth",
    block: "nearest",
  });
}

function selectDesktopContentTab(tab: MediaType) {
  const mode = tab;
  if (!chapterCountOf(mode)) return;
  chapterMode.value = mode;
  setMediaType(tab);
}

function selectMobileMedia(media: MediaType) {
  if (!hasCachedMedia(media)) return;
  setMediaType(media);
  chapterMode.value = media;
  directoryOpen.value = false;
}

onMounted(() => void loadRouteBook());
watch(
  () => route.params.name,
  () => void loadRouteBook(),
);
</script>

<template>
  <section v-if="book" class="detail-page">
    <section class="book-hero" :class="{ 'without-cover': !currentWork.cover }">
      <div
        v-if="currentWork.cover"
        class="hero-art"
        :style="{ backgroundImage: `url('${currentWork.cover}')` }"
        aria-hidden="true"
      />
      <div class="hero-shade" />
      <header class="detail-navigation">
        <button class="nav-circle" title="返回本地书库" @click="goBack">
          <ChevronLeft :size="22" />
        </button>
        <span class="local-label"><LibraryBig :size="14" />本地作品</span>
      </header>

      <div class="hero-copy">
        <h2>{{ currentWork.title }}</h2>
        <div class="hero-meta-row">
          <span class="hero-author">{{ currentWork.author }}</span>
          <span
            v-if="currentWork.score != null"
            class="hero-score"
            :class="scoreToneClass(currentWork.score)"
          >
            <Star :size="14" fill="currentColor" />
            <strong>{{ currentWork.score.toFixed(1) }}</strong>
          </span>
        </div>
      </div>
    </section>

    <section class="desktop-book-workbench">
      <section class="desktop-book-hero-layout">
        <section class="desktop-cover-card glass">
          <div
            class="desktop-cover"
            :class="{ 'without-cover': !currentWork.cover }"
          >
            <img
              v-if="currentWork.cover"
              :src="currentWork.cover"
              :alt="`${currentWork.title} 封面`"
            />
            <BookOpen v-else :size="54" />
          </div>
        </section>
        <section class="desktop-book-overview glass">
          <div class="desktop-book-copy">
            <h2>{{ currentWork.title }}</h2>
            <p class="desktop-author">{{ currentWork.author }}</p>
            <div class="desktop-tags" aria-label="作品标签">
              <span
                v-if="localStatusLabel"
                class="status-tag"
                :class="{ finished: currentWork.isFinished }"
                >{{ localStatusLabel }}</span
              >
              <span v-if="currentWork.typeName">{{
                currentWork.typeName
              }}</span>
              <span v-for="tag in currentWork.tags" :key="tag">{{ tag }}</span>
            </div>
            <button
              class="desktop-description-preview"
              :disabled="!currentWork.description"
              @click="descriptionModalOpen = true"
            >
              <span> {{ currentWork.description || "暂无本地简介" }}</span>
            </button>

            <div class="desktop-primary-actions">
              <button
                class="desktop-start-reading"
                :disabled="
                  !textChapterCount && !audioChapterCount && !comicChapterCount
                "
                @click="startReading"
              >
                <Play :size="18" fill="currentColor" />{{ primaryActionLabel }}
              </button>
              <button
                class="desktop-directory-button"
                @click="focusDesktopDirectory()"
              >
                <ListTree :size="18" />浏览目录
              </button>
            </div>
            <div class="desktop-secondary-actions" aria-label="本地作品操作">
              <button
                v-if="currentWork.id"
                @click="workspace.continueDownload(selectedMediaType)"
              >
                <Download :size="16" />补充下载
              </button>
              <button
                v-if="currentWork.id && selectedMediaType !== 'audio'"
                @click="workspace.startOnlineFromLibrary(selectedMediaType)"
              >
                <ExternalLink :size="16" />在线阅读
              </button>
              <button
                :disabled="Boolean(exporting) || !canExportCurrentMedia"
                @click="exportCurrentMedia"
              >
                <FileDown :size="16" />{{
                  exporting ? "正在导出" : exportActionLabel
                }}
              </button>
            </div>
          </div>
        </section>
        <aside class="desktop-book-facts glass" aria-label="作品状态">
          <div
            class="desktop-score-card"
            :class="scoreToneClass(currentWork.score)"
          >
            <div>
              <small>源站评分</small>
              <strong>{{
                currentWork.score != null ? currentWork.score.toFixed(1) : "0.0"
              }}</strong>
            </div>
            <span><Star :size="15" fill="currentColor" />仅作参考</span>
          </div>
          <div class="desktop-key-metrics" aria-label="作品核心数据">
            <div>
              <BookOpen :size="16" />
              <strong>{{ formatMetric(currentWork.characterCount) }}</strong>
              <span>字数</span>
            </div>
            <div>
              <Ticket :size="16" />
              <strong>{{ formatMetric(currentWork.ticketCount) }}</strong>
              <span>月票</span>
            </div>
            <div>
              <Heart :size="16" fill="currentColor" />
              <strong>{{ formatMetric(currentWork.markCount) }}</strong>
              <span>点赞</span>
            </div>
          </div>
          <div
            v-if="desktopBookFacts.length"
            class="desktop-fact-table"
            aria-label="作品详细信息"
          >
            <div v-for="fact in desktopBookFacts" :key="fact.label">
              <span>{{ fact.label }}</span>
              <strong :title="fact.title">{{ fact.value }}</strong>
            </div>
          </div>
        </aside>
      </section>

      <section class="desktop-content-tabs" aria-label="已保存内容类型">
        <button
          class="desktop-content-tab novel-tab glass"
          :class="{ active: desktopContentTab === 'novel' }"
          :disabled="!textChapterCount"
          @click="selectDesktopContentTab('novel')"
        >
          <BookOpen :size="23" />
          <span
            ><strong>小说</strong
            ><small>{{
              textChapterCount ? `${textChapterCount} 个章节` : "尚无本地章节"
            }}</small></span
          >
          <ChevronRight :size="18" />
        </button>
        <button
          class="desktop-content-tab audio-tab glass"
          :class="{ active: desktopContentTab === 'audio' }"
          :disabled="!audioChapterCount"
          @click="selectDesktopContentTab('audio')"
        >
          <Headphones :size="23" />
          <span
            ><strong>有声</strong
            ><small>{{
              audioChapterCount ? `${audioChapterCount} 条音轨` : "尚无本地音轨"
            }}</small></span
          >
          <ChevronRight :size="18" />
        </button>
        <button
          class="desktop-content-tab comic-tab glass"
          :class="{ active: desktopContentTab === 'comic' }"
          :disabled="!comicChapterCount"
          @click="selectDesktopContentTab('comic')"
        >
          <Images :size="23" />
          <span
            ><strong>漫画</strong
            ><small>{{
              comicChapterCount
                ? `${comicChapterCount} 个章节`
                : "尚无本地漫画章节"
            }}</small></span
          >
          <ChevronRight :size="18" />
        </button>
        <!-- <div class="desktop-content-tabs" aria-label="已保存内容类型">

        </div> -->
      </section>

      <aside
        ref="desktopDirectoryElement"
        class="desktop-directory glass"
        aria-label="本地目录"
      >
        <header>
          <div>
            <small>DOWNLOADED CONTENT</small>
            <h3>章节目录</h3>
          </div>
          <span>{{ desktopDirectoryCount }} 项</span>
        </header>
        <div
          v-if="desktopDirectoryRows.length"
          :ref="setDesktopDirectoryElement"
          class="desktop-directory-list"
          :class="{
            'audio-directory-list': desktopContentTab === 'audio',
            'comic-directory-list': desktopContentTab === 'comic',
          }"
          @scroll.passive="handleDesktopDirectoryScroll"
        >
          <div
            :style="{ height: `${desktopDirectoryWindow.top}px` }"
            aria-hidden="true"
          ></div>
          <template v-for="row in desktopDirectoryWindow.rows" :key="row.key">
            <div
              v-if="row.kind === 'gap'"
              class="directory-volume-gap"
              :style="{ height: `${row.height}px` }"
              aria-hidden="true"
            ></div>
            <h4 v-else-if="row.kind === 'volume'">{{ row.title }}</h4>
            <button
              v-else-if="row.kind === 'text'"
              @click="readChapter(row.id)"
            >
              <span>{{ row.title }}</span
              ><ChevronRight :size="15" />
            </button>
            <button
              v-else-if="row.kind === 'audio'"
              @click="playAudio(row.index)"
            >
              <small>{{ String(row.index + 1).padStart(3, "0") }}</small
              ><span>{{ row.title }}</span
              ><Play :size="14" fill="currentColor" />
            </button>
            <button v-else @click="readComicChapter(row.id)">
              <img
                v-if="row.cover"
                :src="row.cover"
                :alt="`${row.title} 首图`"
              />
              <Images v-else :size="18" />
              <span>{{ row.title }}</span
              ><ChevronRight :size="15" />
            </button>
          </template>
          <div
            :style="{ height: `${desktopDirectoryWindow.bottom}px` }"
            aria-hidden="true"
          ></div>
        </div>
        <div v-else class="desktop-directory-empty">
          <Images v-if="desktopContentTab === 'comic'" :size="27" />
          <ListTree v-else :size="27" />
          {{
            desktopContentTab === "comic" ? "暂无漫画章节" : "暂无已下载内容"
          }}
        </div>
      </aside>

      <ModalShell
        :open="descriptionModalOpen"
        @close="descriptionModalOpen = false"
      >
        <section
          class="description-modal"
          role="dialog"
          aria-modal="true"
          aria-labelledby="full-description-title"
        >
          <header>
            <div>
              <small>ABOUT THIS BOOK</small>
              <h3 id="full-description-title">内容简介</h3>
            </div>
            <button title="关闭简介" @click="descriptionModalOpen = false">
              <X :size="19" />
            </button>
          </header>
          <p>{{ currentWork.description || "暂无本地简介" }}</p>
        </section>
      </ModalShell>
    </section>

    <section class="book-detail-sheet">
      <section class="summary-row" aria-label="作品核心数据">
        <div>
          <BookOpen :size="16" />
          <strong>{{ formatMetric(currentWork.characterCount) }}</strong
          ><span>字数</span>
        </div>
        <div>
          <Ticket :size="16" />
          <strong>{{ formatMetric(currentWork.ticketCount) }}</strong
          ><span>月票</span>
        </div>
        <div>
          <Heart :size="16" fill="currentColor" />
          <strong>{{ formatMetric(currentWork.markCount) }}</strong
          ><span>点赞</span>
        </div>
      </section>

      <section class="format-switches" aria-label="内容类型">
        <button
          v-for="media in mobileSwitchMediaTypes"
          :key="media"
          class="format-switch glass"
          :class="`${media}-switch`"
          :disabled="!hasCachedMedia(media)"
          @click="selectMobileMedia(media)"
        >
          <span class="format-icon">
            <BookOpen v-if="media === 'novel'" :size="23" />
            <Headphones v-else-if="media === 'audio'" :size="23" />
            <ImageIcon v-else :size="23" />
          </span>
          <span class="format-copy"
            ><strong>{{ mediaTitle(media) }}</strong
            ><small>{{ mediaDescription(media) }}</small></span
          >
          <ChevronRight :size="19" />
        </button>
      </section>

      <section
        v-if="
          localStatusLabel || currentWork.typeName || currentWork.tags.length
        "
        class="mobile-book-tags"
        aria-label="作品标签"
      >
        <span
          v-if="localStatusLabel"
          class="status-tag"
          :class="{ finished: currentWork.isFinished }"
          >{{ localStatusLabel }}</span
        >
        <span v-if="currentWork.typeName" class="category-tag">{{
          currentWork.typeName
        }}</span>
        <span v-for="tag in currentWork.tags" :key="tag">{{ tag }}</span>
      </section>

      <section class="book-intro" aria-labelledby="book-intro-title">
        <div class="section-title">
          <span />
          <h3 id="book-intro-title">内容简介</h3>
        </div>
        <p :class="{ expanded: descriptionExpanded }">
          {{ currentWork.description || "暂无本地简介" }}
        </p>
        <button
          v-if="currentWork.description && currentWork.description.length > 104"
          class="expand-description"
          @click="descriptionExpanded = !descriptionExpanded"
        >
          {{ descriptionExpanded ? "收起简介" : "展开简介" }}
        </button>
      </section>

      <button class="mobile-latest-chapter" @click="openDirectory()">
        <span class="latest-badge">最近</span>
        <span class="latest-copy">
          <strong>{{
            currentWork.latestChapterTitle || "暂无最新章节"
          }}</strong>
          <small>{{
            formatSourceDate(
              currentWork.latestChapterTime || currentWork.lastUpdateTime,
            )
          }}</small>
        </span>
        <ChevronRight :size="17" />
      </button>
    </section>

    <nav class="mobile-reading-bar" aria-label="阅读操作">
      <button class="directory-action" @click="openDirectory()">
        <ListTree :size="21" /><span>目录</span>
      </button>
      <button
        v-if="currentWork.id"
        class="directory-action"
        @click="workspace.continueDownload(selectedMediaType)"
      >
        <Download :size="21" /><span>下载</span>
      </button>
      <button
        class="directory-action"
        :disabled="Boolean(exporting) || !canExportCurrentMedia"
        :title="exportActionLabel"
        @click="exportCurrentMedia"
      >
        <FileDown :size="21" /><span>导出</span>
      </button>
      <button
        v-if="currentWork.id && selectedMediaType !== 'audio'"
        class="directory-action"
        title="在线阅读"
        @click="workspace.startOnlineFromLibrary(selectedMediaType)"
      >
        <ExternalLink :size="21" /><span>在线</span>
      </button>
      <button
        class="primary-action"
        :disabled="
          !textChapterCount && !audioChapterCount && !comicChapterCount
        "
        @click="startReading"
      >
        <Play :size="17" fill="currentColor" />{{ primaryActionLabel }}
      </button>
    </nav>

    <DrawerShell
      :open="directoryOpen"
      align="bottom"
      :title="`${mediaTitle(selectedMediaType)}目录`"
      :body-ref="setMobileDirectoryElement"
      @close="directoryOpen = false"
      @scroll="handleMobileDirectoryScroll"
    >
      <template v-if="mobileDirectoryRows.length">
        <div
          :style="{ height: `${mobileDirectoryWindow.top}px` }"
          aria-hidden="true"
        ></div>
        <template v-for="row in mobileDirectoryWindow.rows" :key="row.key">
          <div
            v-if="row.kind === 'gap'"
            class="directory-volume-gap"
            :style="{ height: `${row.height}px` }"
            aria-hidden="true"
          ></div>
          <DirectoryGroupTitle
            v-else-if="row.kind === 'volume'"
            :label="row.title"
          />
          <button
            v-else-if="row.kind === 'text'"
            class="drawer-text-row"
            @click="readChapter(row.id)"
          >
            <strong>{{ row.title }}</strong
            ><ChevronRight :size="16" />
          </button>
          <button
            v-else-if="row.kind === 'audio'"
            class="audio-row"
            @click="playAudio(row.index)"
          >
            <span>{{ String(row.index + 1).padStart(3, "0") }}</span
            ><strong>{{ row.title }}</strong
            ><Headphones :size="16" />
          </button>
          <button
            v-else
            class="comic-directory-row"
            @click="readComicChapter(row.id)"
          >
            <img v-if="row.cover" :src="row.cover" :alt="`${row.title} 首图`" />
            <Images v-else :size="21" />
            <strong>{{ row.title }}</strong
            ><ChevronRight :size="16" />
          </button>
        </template>
        <div
          :style="{ height: `${mobileDirectoryWindow.bottom}px` }"
          aria-hidden="true"
        ></div>
      </template>
      <div v-else class="drawer-state">
        <ListTree :size="24" />暂无已下载内容
      </div>
    </DrawerShell>
  </section>
  <ExportModal
    :open="exportModalOpen"
    :media="selectedMediaType"
    :text-chapter-count="textChapterCount"
    :audio-chapter-count="audioChapterCount"
    :comic-chapter-count="comicChapterCount"
    :exporting="exporting"
    @close="exportModalOpen = false"
    @export="exportFormat"
  />
</template>

<style scoped>
/* Shared page foundation. */
.detail-page {
  height: 100%;
  min-height: 100%;
  overflow-y: auto;
  padding-bottom: 34px;
  color: #483f39;
  /* background: #f7f2ea; */
  background: var(--background-color);
  scrollbar-width: none;
}

.book-hero {
  position: relative;
  overflow: hidden;
  background: #6f625a;
}

.hero-art {
  position: absolute;
  inset: -24px;
  background-position: center 30%;
  background-size: cover;
  filter: saturate(0.92);
}

.hero-shade {
  position: absolute;
  inset: 0;
  background: linear-gradient(180deg, transparent, rgba(25, 23, 20, 0.3));
}

.book-hero.without-cover .hero-shade {
  background: linear-gradient(145deg, #655a4e, #8c715d);
}

.detail-navigation {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.nav-circle,
.local-label,
.directory-action,
.primary-action,
.format-switch,
.desktop-primary-actions button,
.desktop-secondary-actions button,
.expand-description,
.mobile-latest-chapter {
  font: inherit;
}

.nav-circle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 0;
}

.local-label {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.hero-copy h2,
.desktop-book-copy h2,
.section-title h3,
.desktop-directory h3,
.description-modal h3 {
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
}

/* Desktop layout: three-column overview and a full-width local directory. */
@media (min-width: 901px) {
  .detail-page {
    min-height: 100dvh;
  }

  .book-hero {
    min-height: 76px;
    color: #5a4941;
    background: transparent;
  }

  .hero-art,
  .hero-shade,
  .hero-copy {
    display: none;
  }

  .detail-navigation {
    width: min(1180px, calc(100% - 64px));
    margin: 0 auto;
    padding: 18px 0 14px;
  }

  .nav-circle {
    min-height: 38px;
    gap: 4px;
    padding: 0 14px 0 9px;
    border: 1px solid #eaded2;
    border-radius: 12px;
    color: #765f54;
    background: #fffdf8;
  }

  .local-label {
    padding: 7px 10px;
    border: 1px solid #eaded2;
    border-radius: 10px;
    color: #9a7a6c;
    background: #fffdf8;
    font-size: 11px;
  }

  .desktop-book-workbench {
    display: grid;
    width: min(1180px, calc(100% - 64px));
    grid-template-columns: 246px minmax(0, 1fr) 250px;
    gap: 18px;
    align-items: stretch;
    margin: 0 auto 44px;
  }

  .desktop-book-hero-layout {
    display: contents;
  }

  .desktop-cover-card {
    padding: 14px;
    border-radius: 20px;
  }

  .desktop-cover {
    position: relative;
    display: flex;
    width: 100%;
    height: 100%;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    border-radius: 12px;
    color: #a46a54;
    background: #f3dfd1;
  }

  .desktop-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .desktop-book-overview {
    position: relative;
    display: flex;
    min-width: 0;
    padding: 30px 34px;
    border-radius: 20px;
    overflow: hidden;
  }

  .desktop-book-overview:after {
    content: "";
    position: absolute;
    width: 260px;
    height: 260px;
    border: 1px solid rgba(226, 148, 100, 0.28);
    border-radius: 50%;
    right: -90px;
    top: -120px;
    box-shadow:
      0 0 0 35px rgba(226, 148, 100, 0.06),
      0 0 0 70px rgba(226, 148, 100, 0.035);
    pointer-events: none;
  }

  .desktop-book-copy {
    display: flex;
    width: 100%;
    min-width: 0;
    flex-direction: column;
  }

  .desktop-book-copy h2 {
    margin: 0;
    color: #453a34;
    font-size: clamp(28px, 3.3vw, 42px);
    line-height: 1.18;
  }

  .desktop-author {
    margin: 9px 0 0;
    color: #a37b6d;
    font-size: 13px;
  }

  .desktop-tags,
  .mobile-book-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .desktop-tags {
    margin-top: 18px;
  }

  .desktop-tags span,
  .mobile-book-tags span {
    padding: 5px 8px;
    border-radius: 4px;
    color: #8e7669;
    background: #f7ede4;
    font-size: 10px;
  }

  .desktop-tags .status-tag,
  .mobile-book-tags .status-tag {
    color: #fff;
    background: #e18760;
  }

  .desktop-tags .status-tag.finished,
  .mobile-book-tags .status-tag.finished {
    background: #72a28d;
  }

  .desktop-description-preview {
    display: -webkit-box;
    overflow: hidden;
    margin: 11px 0 0;
    padding: 0;
    border: 0;
    color: #766960;
    background: transparent;
    font-size: 13px;
    line-height: 1.8;
    text-align: left;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 4;
  }

  .desktop-description-preview > span {
    display: block;
    white-space: pre-line;
  }

  .desktop-description-preview:hover:not(:disabled) {
    color: #bd6548;
  }

  .desktop-primary-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 9px;
    margin-top: auto;
    padding-top: 11px;
  }

  .desktop-primary-actions button {
    display: inline-flex;
    min-height: 43px;
    align-items: center;
    justify-content: center;
    gap: 7px;
    padding: 0 16px;
    border-radius: 10px;
    font-size: 13px;
  }

  .desktop-start-reading {
    min-width: 132px;
    border: 0;
    color: #fff;
    background: #e47552;
    box-shadow: 0 7px 15px rgba(206, 95, 60, 0.23);
  }

  .desktop-start-reading:hover:not(:disabled) {
    background: #d96746;
  }

  .desktop-start-reading:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .desktop-directory-button {
    min-width: 112px;
    border: 1px solid #e7d8c7;
    color: #785f51;
    background: #fffaf3;
  }

  .desktop-directory-button:hover {
    border-color: #df7658;
    color: #c56246;
  }

  .desktop-secondary-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 22px;
    margin-top: 14px;
  }

  .desktop-secondary-actions button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 0;
    border: 0;
    color: #907365;
    background: transparent;
    font-size: 12px;
  }

  .desktop-secondary-actions button:hover:not(:disabled) {
    color: #c56246;
  }

  .desktop-secondary-actions button:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .desktop-book-facts {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 14px;
    padding: 14px 14px 0 14px;
    border-radius: 20px;
  }

  .desktop-score-card {
    position: relative;
    display: flex;
    min-height: 112px;
    align-items: end;
    justify-content: space-between;
    padding: 17px;
    border-radius: 15px;
    color: #fff;
    background: #e29464;
    box-shadow: 0 9px 18px rgba(201, 110, 67, 0.2);
  }

  .desktop-score-card:after {
    content: "✦";
    position: absolute;
    font-size: 120px;
    right: -30px;
    top: -15px;
    opacity: 0.16;
  }

  .desktop-score-card.score-excellent {
    /* color: #704b0b; */
    background: #e29464;
  }

  .desktop-score-card.score-good {
    background: #9275b3;
  }

  .desktop-score-card.score-average {
    background: #c08a71;
  }

  .desktop-score-card.score-low {
    background: #c87979;
  }

  .desktop-score-card div {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .desktop-score-card small,
  .desktop-score-card > span {
    color: rgba(255, 255, 255, 0.8);
    font-size: 10px;
  }

  .desktop-score-card strong {
    font-family: KaTongFont, "Microsoft YaHei", sans-serif;
    font-size: 46px;
    line-height: 0.9;
  }

  .desktop-score-card > span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
  }

  .desktop-key-metrics {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .desktop-key-metrics div {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 5px;
    padding: 0 8px;
  }

  .desktop-key-metrics div + div {
    border-left: 1px solid #eadfd1;
  }

  .desktop-key-metrics svg {
    color: #d87958;
  }

  .desktop-key-metrics strong {
    overflow: hidden;
    color: #55483f;
    font-family: KaTongFont, "Microsoft YaHei", sans-serif;
    font-size: 18px;
    line-height: 1.1;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .desktop-key-metrics span {
    color: #a09083;
    font-size: 10px;
  }

  .desktop-fact-table {
    display: flex;
    flex: 1 1 auto;
    margin-top: 2px;
    border-top: 1px solid #eadfd1;
    flex-direction: column;
  }

  .desktop-fact-table > div {
    display: flex;
    min-width: 0;
    min-height: 37px;
    flex: 1 1 0;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    border-bottom: 1px solid #eadfd1;
  }

  .desktop-fact-table > div:last-child {
    border-bottom: 0;
  }

  .desktop-fact-table span {
    flex: 0 0 auto;
    color: #a09083;
    font-size: 11px;
  }

  .desktop-fact-table strong {
    min-width: 0;
    overflow: hidden;
    color: #6f5a4e;
    font-family: KaTongFont, "Microsoft YaHei", sans-serif;
    font-size: 13px;
    line-height: 1.2;
    text-align: right;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .desktop-content-tabs {
    display: flex;
    grid-column: 1 / -1;
    gap: 12px;
  }

  .desktop-content-tab {
    display: flex;
    min-width: 0;
    min-height: 82px;
    flex: 1 1 0;
    align-items: center;
    gap: 12px;
    padding: 14px 17px;
    border-radius: 16px;
    text-align: left;
  }

  .desktop-content-tab > svg:first-child {
    width: 48px;
    height: 48px;
    flex: 0 0 auto;
    padding: 12px;
    border-radius: 50%;
    color: #c56246;
    background: #f3dfd1;
  }

  .desktop-content-tab > span {
    display: flex;
    min-width: 0;
    flex: 1 1 auto;
    flex-direction: column;
    gap: 5px;
  }

  .desktop-content-tab strong {
    font-size: 16px;
  }

  .desktop-content-tab small {
    overflow: hidden;
    color: #9b8b7e;
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .desktop-content-tab > svg:last-child {
    flex: 0 0 auto;
    margin-left: auto;
    color: #a89786;
  }

  .desktop-content-tab:hover:not(:disabled),
  .desktop-content-tab.active {
    border-color: #e3a078;
    color: #bd6548;
    /* box-shadow: 0 8px 18px rgba(188, 112, 77, 0.12); */
  }

  .desktop-content-tab:disabled {
    cursor: not-allowed;
    opacity: 0.62;
  }

  .desktop-directory {
    display: flex;
    min-height: 390px;
    max-height: 620px;
    grid-column: 1 / -1;
    flex-direction: column;
    overflow: hidden;
    border-radius: 16px;
  }

  .desktop-directory > header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 19px 20px 13px;
  }

  .desktop-directory header small {
    color: #b59885;
    font: 10px monospace;
    letter-spacing: 0.08em;
  }

  .desktop-directory h3 {
    margin: 4px 0 0;
    color: #4b3e35;
    font-size: 20px;
  }

  .desktop-directory > header > span {
    padding: 5px 8px;
    border-radius: 5px;
    color: #a17665;
    background: #f9ede5;
    font-size: 10px;
  }

  .desktop-directory-list {
    overflow-y: auto;
    padding-bottom: 8px;
    scrollbar-color: #e9b697 transparent;
  }

  .desktop-directory-list h4 {
    display: block;
    height: 40px;
    margin: 0;
    padding: 0 18px;
    overflow: hidden;
    color: #9c8d7d;
    font-size: 11px;
    line-height: 40px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .desktop-directory-list button {
    display: flex;
    width: 100%;
    height: 40px;
    min-height: 40px;
    align-items: center;
    gap: 8px;
    padding: 0 18px;
    border: 0;
    border-top: 1px solid #f3ede3;
    color: #5c5045;
    background: transparent;
    text-align: left;
  }

  .desktop-directory-list button:hover {
    color: #c56446;
    background: #fff8ef;
  }

  .desktop-directory-list button span {
    min-width: 0;
    flex: 1 1 auto;
    overflow: hidden;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .desktop-directory-list button svg {
    flex: 0 0 auto;
    margin-left: auto;
    color: #a89786;
  }

  .audio-directory-list small {
    flex: 0 0 30px;
    color: #b39480;
    font: 10px monospace;
  }

  .desktop-directory-empty {
    display: flex;
    min-height: 200px;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    color: #a59889;
    font-size: 12px;
  }

  .book-detail-sheet,
  .mobile-reading-bar,
  .mobile-book-tags,
  .mobile-latest-chapter {
    display: none;
  }
}

/* Mobile layout: tall cover hero, overlapping detail sheet, and action dock. */
@media (max-width: 900px) {
  .detail-page {
    min-height: 100dvh;
    padding-bottom: calc(84px + env(safe-area-inset-bottom));
    background: #fff;
  }

  .book-hero {
    height: clamp(286px, 78vw, 360px);
    color: #fff;
  }

  .hero-art {
    inset: -14px;
    background-position: center 28%;
  }

  .detail-navigation {
    padding: calc(5px + env(safe-area-inset-top)) 16px 14px;
  }

  .nav-circle {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    color: #fff;
    background: rgba(32, 29, 24, 0.48);
  }

  .local-label {
    padding: 8px 10px;
    border: 1px solid rgba(255, 255, 255, 0.25);
    border-radius: 15px;
    color: rgba(255, 255, 255, 0.92);
    background: rgba(39, 34, 28, 0.28);
    font-size: 11px;
  }

  .hero-copy {
    position: absolute;
    z-index: 1;
    right: 20px;
    bottom: 50px;
    left: 20px;
    text-shadow: 0 2px 11px rgba(0, 0, 0, 0.34);
  }

  .hero-copy h2 {
    display: -webkit-box;
    overflow: hidden;
    margin: 0;
    font-size: clamp(24px, 7vw, 31px);
    line-height: 1.18;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
  }

  .hero-meta-row {
    display: flex;
    min-width: 0;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 8px;
  }

  .hero-author {
    display: block;
    min-width: 0;
    overflow: hidden;
    color: rgba(255, 255, 255, 0.84);
    font-size: 13px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hero-score {
    display: inline-flex;
    min-width: 76px;
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
    gap: 5px;
    margin: 0;
    padding: 6px 9px;
    border-radius: 8px;
    color: #704b0b;
    background: #f3d477;
    font-size: 11px;
    font-weight: 700;
    text-shadow: none;
  }

  .hero-score strong {
    color: inherit;
    font-size: 15px;
    line-height: 1;
  }

  .hero-score.score-excellent {
    color: #704b0b;
    background: #f3d477;
  }

  .hero-score.score-good {
    color: #174f3b;
    background: #a9d7c1;
  }

  .hero-score.score-average {
    color: #214f70;
    background: #b7d6e9;
  }

  .hero-score.score-low {
    color: #713939;
    background: #e7b5b5;
  }

  .desktop-book-workbench {
    display: none;
  }

  .book-detail-sheet {
    position: relative;
    z-index: 2;
    display: block;
    width: 100%;
    margin-top: -31px;
    padding: 25px 20px 18px;
    border: 0;
    border-radius: 28px 28px 0 0;
    background: linear-gradient(
      180deg,
      rgba(255, 255, 255, 0.58) 0,
      rgba(255, 255, 255, 0.82) 22px,
      #fff 31px
    );
    box-shadow: none;
    backdrop-filter: blur(19px);
    -webkit-backdrop-filter: blur(19px);
  }

  .summary-row {
    display: flex;
    padding-bottom: 22px;
    border-bottom: 1px solid #e8e2d3;
  }

  .summary-row div {
    display: flex;
    min-width: 0;
    flex: 1 1 0;
    flex-direction: column;
    gap: 5px;
    padding: 0 9px;
  }

  .summary-row div + div {
    border-left: 1px solid #e8e2d3;
  }

  .summary-row strong,
  .summary-row span {
    display: block;
  }

  .summary-row strong {
    min-width: 0;
    overflow: hidden;
    color: #39342e;
    font-family: KaTongFont, "Microsoft YaHei", sans-serif;
    font-size: clamp(22px, 7vw, 29px);
    line-height: 1;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .summary-row svg {
    color: #d87958;
  }

  .summary-row span {
    color: #938c80;
    font-size: 10px;
    white-space: nowrap;
  }

  .mobile-book-tags {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }

  .mobile-book-tags span {
    display: inline-flex;
    min-height: 24px;
    align-items: center;
    max-width: 100%;
    padding: 0 8px;
    overflow: hidden;
    border: 0;
    border-radius: 999px;
    color: #483f39;
    background: #f4f4f4;
    font-size: 12px;
    line-height: 1;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mobile-book-tags .status-tag {
    color: #fff;
    background: #e86e4c;
  }

  .mobile-book-tags .status-tag.finished {
    color: #483f39;
    background: #f0f0f0;
  }

  .mobile-book-tags .category-tag {
    color: #483f39;
    background: #f4f4f4;
  }

  .format-switches {
    display: flex;
    gap: 9px;
    padding: 19px 0;
  }

  .format-switch {
    display: flex;
    min-width: 0;
    height: 48px;
    flex: 1 1 0;
    align-items: center;
    gap: 8px;
    padding: 5px 10px;
    overflow: hidden;
    --glass-lift: 0 10px 22px -12px rgba(85, 73, 53, 0.28);
    border-radius: 14px;
    color: #4f4940;
    text-align: left;
  }

  .format-switch:disabled {
    cursor: not-allowed;
    opacity: 0.58;
  }

  .format-icon {
    display: inline-flex;
    width: 30px;
    height: 30px;
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
    border-radius: 11px;
  }

  .format-copy {
    display: flex;
    min-width: 0;
    flex: 1 1 auto;
    flex-direction: column;
    gap: 5px;
  }

  .format-copy strong,
  .format-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .format-copy strong {
    font-size: 14px;
  }

  .format-copy small {
    display: none;
  }

  .audio-switch {
    color: #60471c;
    background: rgba(242, 194, 72, 0.46);
  }

  .audio-switch .format-icon {
    color: #79470a;
    background: rgba(255, 244, 205, 0.65);
  }

  .audio-switch .format-copy small,
  .audio-switch > svg {
    color: #8b6826;
  }

  .novel-switch {
    color: #245e7d;
    background: rgba(167, 215, 239, 0.52);
  }

  .novel-switch .format-icon {
    color: #236c91;
    background: rgba(239, 250, 255, 0.72);
  }

  .novel-switch .format-copy small,
  .novel-switch > svg {
    color: #4b7f99;
  }

  .comic-switch {
    color: #433860;
    background: rgba(184, 162, 221, 0.42);
  }

  .comic-switch .format-icon {
    color: #604a91;
    background: rgba(246, 240, 255, 0.64);
  }

  .comic-switch .format-copy small,
  .comic-switch > svg {
    color: #776592;
  }

  .format-switch > svg {
    flex: 0 0 auto;
    width: 16px;
    height: 16px;
    margin-left: 0;
  }

  .book-intro {
    padding: 20px 0 19px;
    border-top: 1px solid #e8e2d3;
    border-bottom: 1px solid #e8e2d3;
  }

  .mobile-book-tags + .book-intro {
    margin-top: 19px;
  }

  .section-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .section-title > span {
    width: 4px;
    height: 20px;
    border-radius: 2px;
    background: #e87c54;
  }

  .section-title h3 {
    margin: 0;
    color: #433c35;
    font-size: 19px;
  }

  .book-intro p {
    display: -webkit-box;
    overflow: hidden;
    margin: 12px 0 0;
    color: #716b61;
    font-size: 14px;
    line-height: 1.82;
    white-space: pre-line;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
  }

  .book-intro p.expanded {
    display: block;
  }

  .expand-description {
    padding: 7px 0 0;
    border: 0;
    color: #c46648;
    background: transparent;
    font-size: 12px;
  }

  .mobile-latest-chapter {
    display: flex;
    width: 100%;
    min-height: 44px;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
    padding: 0 12px;
    border: 0;
    border-radius: 12px;
    color: #444;
    background: #f4f4f4;
    box-shadow: none;
    text-align: left;
  }

  .latest-badge {
    display: inline-flex;
    height: 20px;
    align-items: center;
    padding: 0 8px;
    border-radius: 999px;
    color: #fff;
    background: #e86e4c;
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
  }

  .latest-copy {
    display: flex;
    min-width: 0;
    flex: 1 1 auto;
    align-items: baseline;
    gap: 9px;
  }

  .latest-copy strong {
    min-width: 0;
    overflow: hidden;
    color: #808080;
    font-size: 13px;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .latest-copy small {
    flex: 0 0 auto;
    color: #777;
    font-size: 12px;
  }

  .mobile-latest-chapter > svg {
    flex: 0 0 auto;
    color: #8a8a8a;
  }

  .mobile-reading-bar {
    position: fixed;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 20;
    display: flex;
    align-items: stretch;
    gap: 6px;
    padding: 8px 12px calc(8px + env(safe-area-inset-bottom));
    border-top: 1px solid #e9dfd0;
    background: #fff;
    box-shadow: 0 -7px 22px rgba(58, 48, 38, 0.1);
    backdrop-filter: none;
  }

  .directory-action {
    display: flex;
    min-height: 48px;
    flex: 0 0 48px;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    padding: 0;
    border: 0;
    color: #625a4e;
    background: transparent;
    font: inherit;
    font-size: 12px;
  }

  .directory-action:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .primary-action {
    display: inline-flex;
    min-height: 48px;
    flex: 1 1 0;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: 0;
    border-radius: 999px;
    color: #fff;
    background: #eb6545;
    box-shadow: 0 5px 13px rgba(207, 91, 62, 0.25);
    font-size: 14px;
    font-weight: 700;
  }

  .primary-action:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }
}

/* Modal 与抽屉共用同一套暖白面板与标题排版。 */
.description-modal {
  width: min(680px, 100%);
  max-height: min(72dvh, 620px);
  overflow-y: auto;
  padding: 26px 30px 30px;
  border: 1px solid #eaded1;
  border-radius: 20px;
  background: #fffdf8;
  box-shadow: 0 22px 55px rgba(56, 43, 34, 0.22);
}

.description-modal header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding-bottom: 15px;
  border-bottom: 1px solid #eee5d9;
}

.description-modal header small {
  color: #b59885;
  font: 10px monospace;
  letter-spacing: 0.08em;
}

.description-modal h3 {
  margin: 4px 0 0;
  color: #4c4037;
  font-size: 22px;
}

.description-modal header button {
  display: inline-flex;
  width: 34px;
  height: 34px;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 50%;
  color: #79695e;
  background: #f6efe7;
}

.description-modal p {
  margin: 20px 0 0;
  color: #655a52;
  font-size: 14px;
  line-height: 2;
  white-space: pre-line;
}

/* 目录抽屉内容：面板、标题栏与滚动区由 DrawerShell 统一提供。 */
.directory-volume-gap {
  background: #f7f3ea;
}

.drawer-text-row,
.audio-row {
  display: flex;
  width: 100%;
  height: 48px;
  min-height: 48px;
  align-items: center;
  gap: 8px;
  padding: 0 20px;
  border: 0;
  border-top: 1px solid #f0ebdf;
  color: #504a41;
  background: transparent;
  text-align: left;
}

.drawer-text-row:hover,
.audio-row:hover {
  color: #bd6548;
  background: #fff9f1;
}

.drawer-text-row strong,
.audio-row strong {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  font-size: 13px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.drawer-text-row svg,
.audio-row svg {
  flex: 0 0 auto;
  margin-left: auto;
  color: #a99f90;
}

.audio-row span {
  flex: 0 0 38px;
  color: #ad9f8d;
  font: 11px monospace;
}

.comic-directory-list button,
.comic-directory-row {
  height: 62px;
  min-height: 62px;
}

.comic-directory-list img,
.comic-directory-row img {
  width: 64px;
  height: 44px;
  flex: 0 0 auto;
  border-radius: 4px;
  object-fit: cover;
}

.comic-directory-row {
  display: flex;
  width: 100%;
  height: 66px;
  min-height: 66px;
  align-items: center;
  gap: 11px;
  padding: 7px 20px;
  border: 0;
  border-top: 1px solid #f0ebdf;
  color: #504a41;
  background: transparent;
  text-align: left;
}

.comic-directory-row strong {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  font-size: 13px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.comic-directory-row > svg:last-child {
  flex: 0 0 auto;
  margin-left: auto;
  color: #a99f90;
}
</style>
