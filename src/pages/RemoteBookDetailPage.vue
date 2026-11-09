<script setup lang="ts">
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  computed,
  nextTick,
  ref,
  watch,
  type ComponentPublicInstance,
} from "vue";
import {
  BookOpen,
  Bookmark,
  ChevronLeft,
  ChevronRight,
  Download,
  ExternalLink,
  Heart,
  Headphones,
  Image as ImageIcon,
  LibraryBig,
  LoaderCircle,
  ListTree,
  LockKeyholeOpen,
  MessageCircle,
  Play,
  Star,
  Ticket,
  X,
} from "@lucide/vue";
import BookCommentCard from "../components/BookCommentCard.vue";
import DiscussionDanmaku from "../components/DiscussionDanmaku.vue";
import DiscussionTabs from "../components/DiscussionTabs.vue";
import ModalShell from "../components/ModalShell.vue";
import { useBookDiscussions } from "../composables/useBookDiscussions";
import { useVirtualRows } from "../composables/useVirtualRows";
import { reportError } from "../composables/useToast";
import type { BookComment, Chapter, MediaType } from "../types";
import { formatMetric, formatSourceDate, scoreToneClass } from "../utils/format";

import { useOnline, usePicker, useRemote, useWorkspace } from "../desk";

const online = useOnline();
const picker = usePicker();
const remote = useRemote();
const workspace = useWorkspace();

const novel = computed(() => remote.remoteBook.value);
const mediaTypes: MediaType[] = ["novel", "audio", "comic"];
const descriptionExpanded = ref(false);
const descriptionModalOpen = ref(false);
const {
  activeTab: discussionTab,
  current: discussion,
  preview: discussionPreview,
  highlights: discussionHighlights,
  selectTab: selectDiscussionTab,
  reload: reloadDiscussion,
} = useBookDiscussions(novel, 6);
const mode = computed<MediaType>(() =>
  novel.value?.bookshelfType === "audio"
    ? "audio"
    : novel.value?.bookshelfType === "comic"
      ? "comic"
      : "novel",
);
const remoteMedia = computed(() => remote.remoteMedia.value);
const remoteMediaDetecting = computed(() => remote.remoteMediaDetecting.value);
const mobileSwitchMediaTypes = computed(() =>
  mediaTypes.filter((media) => media !== selectedMedia.value),
);
const directoryOpen = ref(false);
const directoryElement = ref<HTMLElement>();
const directoryLoading = computed(() => online.directoryLoading.value);
const directoryError = computed(() => online.directoryError.value);
const textVolumes = computed(() => online.textVolumes.value);
const audioChapters = computed(() => online.audioChapters.value);
const comicChapters = computed(() => online.comicChapters.value);
const audioVolumes = computed(() => groupChaptersByVolume(audioChapters.value));
const comicVolumes = computed(() => groupChaptersByVolume(comicChapters.value));
const selectedMedia = computed<MediaType>(
  () => novel.value?.bookshelfType || "novel",
);
const currentDirectoryCount = computed(() =>
  mode.value === "novel"
    ? textVolumes.value.reduce(
        (count, volume) => count + volume.chapters.length,
        0,
      )
    : mode.value === "audio"
      ? audioChapters.value.length
      : comicChapters.value.length,
);
const currentMediaLabel = computed(() =>
  selectedMedia.value === "audio"
    ? "有声"
    : selectedMedia.value === "comic"
      ? "漫画"
      : "小说",
);
type RemoteDirectoryChapter = {
  id: number;
  title: string;
  isVip?: boolean;
  isUnlocked?: boolean | "unknown";
};
type RemoteDesktopRow =
  | { kind: "gap"; key: string; height: number }
  | { kind: "volume"; key: string; title: string; height: number }
  | {
      kind: "chapters";
      key: string;
      chapters: RemoteDirectoryChapter[];
      height: number;
    };
type RemoteDrawerRow =
  | { kind: "gap"; key: string; height: number }
  | { kind: "volume"; key: string; title: string; height: number }
  | {
      kind: "chapter";
      key: string;
      id: number;
      title: string;
      isVip?: boolean;
      isUnlocked?: boolean | "unknown";
      index?: number;
      height: number;
    };
type RemoteDirectoryVolume = {
  title: string;
  chapters: RemoteDirectoryChapter[];
};

/**
 * 把当前内容类型的在线目录归一成分卷结构。
 *
 * 三种内容的字段名不同：文字章节是 `chapId` + `accessState === "available"`，
 * 音视频是 `id` + `isUnlocked`。这份映射原先在桌面与抽屉两个构建器里各写一遍，
 * 改漏一处不会报错，只会让 VIP 标记或解锁状态显示错误。
 */
const directoryVolumes = computed<RemoteDirectoryVolume[]>(() => {
  if (mode.value === "novel") {
    return textVolumes.value.map((volume) => ({
      title: volume.title,
      chapters: volume.chapters.map((chapter) => ({
        id: chapter.chapId,
        title: chapter.title,
        isVip: chapter.isVip,
        isUnlocked: chapter.accessState === "available",
      })),
    }));
  }

  const volumes =
    mode.value === "audio" ? audioVolumes.value : comicVolumes.value;
  return volumes.map((volume) => ({
    title: volume.title,
    chapters: volume.chapters.map((chapter) => ({
      id: chapter.id,
      title: chapter.title,
      isVip: chapter.isVip,
      isUnlocked: chapter.isUnlocked,
    })),
  }));
});

const desktopDirectoryRows = computed<RemoteDesktopRow[]>(() => {
  const rows: RemoteDesktopRow[] = [];
  directoryVolumes.value.forEach((volume, volumeIndex) => {
    if (volumeIndex > 0)
      rows.push({
        kind: "gap",
        key: `desktop-gap-${volumeIndex}`,
        height: 12,
      });
    rows.push({
      kind: "volume",
      key: `desktop-volume-${volumeIndex}`,
      title: volume.title,
      height: 40,
    });
    for (let index = 0; index < volume.chapters.length; index += 3)
      rows.push({
        kind: "chapters",
        key: `desktop-chapters-${volumeIndex}-${index}`,
        chapters: volume.chapters.slice(index, index + 3),
        height: 41,
      });
  });
  return rows;
});

const drawerDirectoryRows = computed<RemoteDrawerRow[]>(() => {
  const rows: RemoteDrawerRow[] = [];
  if (mode.value === "novel") {
    directoryVolumes.value.forEach((volume, volumeIndex) => {
      if (volumeIndex > 0)
        rows.push({
          kind: "gap",
          key: `drawer-gap-${volumeIndex}`,
          height: 9,
        });
      rows.push({
        kind: "volume",
        key: `drawer-volume-${volumeIndex}`,
        title: volume.title,
        height: 42,
      });
      for (const chapter of volume.chapters)
        rows.push({
          kind: "chapter",
          key: `drawer-text-${volumeIndex}-${chapter.id}`,
          id: chapter.id,
          title: chapter.title,
          isVip: chapter.isVip,
          isUnlocked: chapter.isUnlocked,
          height: 48,
        });
    });
    return rows;
  }

  // 音视频不展示分卷标题，直接平铺；index 用于播放队列定位。
  const chapters = directoryVolumes.value.flatMap((volume) => volume.chapters);
  chapters.forEach((chapter, index) =>
    rows.push({
      kind: "chapter",
      key: `drawer-${mode.value}-${chapter.id}`,
      id: chapter.id,
      title: chapter.title,
      isVip: chapter.isVip,
      isUnlocked: chapter.isUnlocked,
      index: mode.value === "audio" ? index : undefined,
      height: 48,
    }),
  );
  return rows;
});
const {
  container: desktopDirectoryList,
  windowed: desktopDirectoryWindow,
  handleScroll: handleDesktopDirectoryScroll,
  reset: resetDesktopDirectory,
} = useVirtualRows(() => desktopDirectoryRows.value);
const {
  container: drawerDirectoryList,
  windowed: drawerDirectoryWindow,
  handleScroll: handleDrawerDirectoryScroll,
  reset: resetDrawerDirectory,
} = useVirtualRows(() => drawerDirectoryRows.value);
function setDesktopDirectoryElement(
  element: Element | ComponentPublicInstance | null,
) {
  desktopDirectoryList.value =
    element instanceof HTMLElement ? element : undefined;
}
function setDrawerDirectoryElement(
  element: Element | ComponentPublicInstance | null,
) {
  drawerDirectoryList.value =
    element instanceof HTMLElement ? element : undefined;
}
const isFavorite = computed(() => remote.isRemoteFavorite(novel.value));
const favoriteBusy = computed(() => remote.isRemoteFavoriteLoading(novel.value));
const remoteStatusLabel = computed(() =>
  novel.value?.isFinish == null
    ? ""
    : novel.value.isFinish
      ? "已完结"
      : "连载中",
);
const onlineActionLabel = computed(() =>
  mode.value === "audio" ? "在线收听" : "在线阅读",
);
const bookFacts = computed(() => {
  const work = novel.value;
  if (!work) return [];
  const facts: Array<{ label: string; value: string }> = [];
  const updatedAt = work.latestChapterTime || work.lastUpdateTime;
  if (updatedAt)
    facts.push({ label: "最近更新", value: formatSourceDate(updatedAt) });
  if (work.viewCount != null)
    facts.push({ label: "热度", value: formatMetric(work.viewCount) });
  if (work.pointCount != null)
    facts.push({ label: "人气", value: formatMetric(work.pointCount) });
  if (work.favoriteCount != null)
    facts.push({ label: "收藏", value: formatMetric(work.favoriteCount) });
  return facts;
});

function goBack() {
  workspace.back();
}
function openComments() {
  workspace.openComments(discussionTab.value);
}
function openComment(comment: BookComment) {
  workspace.openComments(discussionTab.value, comment.commentId);
}
function openDownload() {
  if (novel.value) void picker.openChapterPicker(novel.value, mode.value);
}
function startOnline(chapterId?: number) {
  const work = novel.value;
  if (!work) return;
  workspace.startOnlineReading(work, mode.value, chapterId);
}

function favoriteWork() {
  const work = novel.value;
  if (work) void remote.toggleRemoteFavorite(work);
}

function selectMedia(media: MediaType) {
  if (remoteMedia.value[media]) remote.selectRemoteMedia(media);
}

function mediaTitle(media: MediaType) {
  return media === "audio"
    ? "有声小说"
    : media === "comic"
      ? "漫画改编"
      : "小说原著";
}

function mediaDescription(media: MediaType) {
  if (remoteMedia.value[media]) {
    return media === selectedMedia.value ? "当前查看" : "可切换查看";
  }
  return remoteMediaDetecting.value ? "正在探测" : "暂未发现";
}

async function ensureDirectory(force = false) {
  const work = novel.value;
  if (!work) return;
  try {
    await online.primeOnlineDirectory(work, mode.value, force);
  } catch {
    // 错误保存在共享目录状态中，由详情页和阅读器共同展示。
  }
}

async function showDirectory() {
  const isMobile = window.matchMedia("(max-width: 900px)").matches;
  resetDrawerDirectory();
  directoryOpen.value = isMobile;
  if (isMobile) return;
  await nextTick();
  directoryElement.value?.scrollIntoView({
    behavior: "smooth",
    block: "nearest",
  });
}

async function openOfficialPage() {
  const work = novel.value;
  if (!work) return;
  const url =
    work.bookshelfType === "comic" && work.sourcePath
      ? `https://manhua.sfacg.com/mh/${work.sourcePath}/`
      : `https://book.sfacg.com/Novel/${work.novelId}/`;
  try {
    await openUrl(url);
  } catch (error) {
    reportError(error, "打开官方页面失败");
  }
}

watch(
  () =>
    [
      novel.value?.novelId,
      novel.value?.mediaId,
      novel.value?.sourcePath,
      mode.value,
    ] as const,
  () => {
    descriptionExpanded.value = false;
    descriptionModalOpen.value = false;
    directoryOpen.value = false;
    resetDesktopDirectory();
    resetDrawerDirectory();
    void ensureDirectory();
  },
  { immediate: true },
);
function groupChaptersByVolume(chapters: Chapter[]) {
  const groups = new Map<string, Chapter[]>();
  for (const chapter of chapters) {
    const volume = chapter.volume?.trim() || "未分卷";
    const entries = groups.get(volume) || [];
    entries.push(chapter);
    groups.set(volume, entries);
  }
  return Array.from(groups, ([title, groupedChapters]) => ({
    title,
    chapters: groupedChapters,
  }));
}
</script>

<template>
  <section v-if="novel" class="detail-page">
    <section class="book-hero" :class="{ 'without-cover': !novel.novelCover }">
      <div
        v-if="novel.novelCover"
        class="hero-art"
        :style="{ backgroundImage: `url('${novel.novelCover}')` }"
        aria-hidden="true"
      />
      <div class="hero-shade" />
      <header class="detail-navigation">
        <button class="nav-circle" title="返回上一页" @click="goBack">
          <ChevronLeft :size="22" /><span class="desktop-nav-label">返回</span>
        </button>
        <span class="local-label"><LibraryBig :size="14" />远程作品</span>
      </header>
      <div class="hero-copy">
        <h2>{{ novel.novelName }}</h2>
        <div class="hero-meta-row">
          <span class="hero-author">{{ novel.authorName || "未知作者" }}</span
          ><span
            v-if="novel.score != null"
            class="hero-score"
            :class="scoreToneClass(novel.score)"
            ><Star :size="14" fill="currentColor" /><strong>{{
              novel.score.toFixed(1)
            }}</strong></span
          >
        </div>
      </div>
    </section>

    <section class="desktop-book-workbench">
      <section class="desktop-cover-card glass">
        <div
          class="desktop-cover"
          :class="{ 'without-cover': !novel.novelCover }"
        >
          <img
            v-if="novel.novelCover"
            :src="novel.novelCover"
            :alt="`${novel.novelName} 封面`"
          /><BookOpen v-else :size="54" />
        </div>
      </section>
      <section class="desktop-book-overview glass">
        <div class="desktop-book-copy">
          <h2>{{ novel.novelName }}</h2>
          <p class="desktop-author">{{ novel.authorName || "未知作者" }}</p>
          <div
            v-if="
              remoteStatusLabel ||
              novel.typeName ||
              novel.bookshelfName ||
              novel.tags?.length
            "
            class="desktop-tags"
            aria-label="作品标签"
          >
            <span
              v-if="remoteStatusLabel"
              class="status-tag"
              :class="{ finished: novel.isFinish }"
              >{{ remoteStatusLabel }}</span
            ><span v-if="novel.typeName || novel.bookshelfName">{{
              novel.typeName || novel.bookshelfName
            }}</span
            ><span v-for="tag in novel.tags || []" :key="tag">{{ tag }}</span>
          </div>
          <button
            class="desktop-description-preview"
            :disabled="!novel.description"
            @click="descriptionModalOpen = true"
          >
            <span>{{ novel.description || "暂无简介" }}</span>
          </button>
          <div class="desktop-primary-actions">
            <button class="desktop-start-reading" @click="startOnline()">
              <Play :size="18" />{{ onlineActionLabel }}
            </button>
            <button
              class="desktop-favorite"
              :class="{ favorited: isFavorite }"
              :disabled="favoriteBusy"
              :aria-label="isFavorite ? '取消收藏' : '收藏'"
              @click="favoriteWork"
            >
              <LoaderCircle v-if="favoriteBusy" class="spin" :size="17" />
              <Bookmark
                v-else
                :size="17"
                :fill="isFavorite ? 'currentColor' : 'none'"
              />{{ isFavorite ? "已收藏" : "收藏" }}
            </button>
            <button class="desktop-action" @click="openDownload">
              <Download :size="17" />下载
            </button>
            <button class="desktop-action" @click="openOfficialPage">
              <ExternalLink :size="17" />官方网页
            </button>
          </div>
          <div class="desktop-secondary-actions">
            <LoaderCircle v-if="remoteMediaDetecting" class="spin" :size="16" />
            <ListTree v-else :size="16" />
            {{
              remoteMediaDetecting
                ? "正在探测有声与漫画"
                : "目录仅显示当前作品类型，其他类型请切换查看"
            }}
          </div>
        </div>
      </section>
      <aside class="desktop-book-facts glass" aria-label="作品信息">
        <div class="desktop-score-card" :class="scoreToneClass(novel.score)">
          <div>
            <small>源站评分</small
            ><strong>{{
              novel.score != null ? novel.score.toFixed(1) : "--"
            }}</strong>
          </div>
          <span><Star :size="15" fill="currentColor" />仅作参考</span>
        </div>
        <div class="desktop-key-metrics" aria-label="作品核心数据">
          <div>
            <BookOpen :size="16" /><strong>{{
              formatMetric(novel.characterCount)
            }}</strong
            ><span>字数</span>
          </div>
          <div>
            <Ticket :size="16" /><strong>{{
              formatMetric(novel.ticketCount)
            }}</strong
            ><span>月票</span>
          </div>
          <div>
            <Heart :size="16" fill="currentColor" /><strong>{{
              formatMetric(novel.markCount)
            }}</strong
            ><span>点赞</span>
          </div>
        </div>
        <div
          v-if="bookFacts.length"
          class="desktop-fact-table"
          aria-label="作品详细信息"
        >
          <div v-for="fact in bookFacts" :key="fact.label">
            <span>{{ fact.label }}</span
            ><strong>{{ fact.value }}</strong>
          </div>
        </div>
      </aside>
    </section>

    <ModalShell
      :open="descriptionModalOpen"
      @close="descriptionModalOpen = false"
    >
      <section
        class="description-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="remote-full-description-title"
      >
        <header>
          <div>
            <small>ABOUT THIS BOOK</small>
            <h3 id="remote-full-description-title">内容简介</h3>
          </div>
          <button title="关闭简介" @click="descriptionModalOpen = false">
            <X :size="19" />
          </button>
        </header>
        <p>{{ novel.description || "暂无简介" }}</p>
      </section>
    </ModalShell>

    <section class="desktop-content-tabs" aria-label="远程作品类型">
      <button
        v-for="media in mediaTypes"
        :key="media"
        class="desktop-content-tab glass"
        :class="{ active: selectedMedia === media }"
        :disabled="!remoteMedia[media]"
        :title="mediaDescription(media)"
        @click="selectMedia(media)"
      >
        <Headphones v-if="media === 'audio'" :size="23" />
        <ImageIcon v-else-if="media === 'comic'" :size="23" />
        <BookOpen v-else :size="23" />
        <span>
          <strong>{{
            media === "audio" ? "有声" : media === "comic" ? "漫画" : "小说"
          }}</strong>
          <small>{{ mediaDescription(media) }}</small>
        </span>
        <ChevronRight :size="18" />
      </button>
    </section>

    <section class="book-detail-sheet">
      <section class="summary-row" aria-label="作品核心数据">
        <div>
          <BookOpen :size="16" /><strong>{{
            formatMetric(novel.characterCount)
          }}</strong
          ><span>字数</span>
        </div>
        <div>
          <Ticket :size="16" /><strong>{{
            formatMetric(novel.ticketCount)
          }}</strong
          ><span>月票</span>
        </div>
        <div>
          <Heart :size="16" fill="currentColor" /><strong>{{
            formatMetric(novel.markCount)
          }}</strong
          ><span>点赞</span>
        </div>
      </section>
      <section class="format-switches" aria-label="内容类型">
        <button
          v-for="media in mobileSwitchMediaTypes"
          :key="media"
          class="format-switch glass"
          :class="`${media}-switch`"
          :disabled="!remoteMedia[media]"
          @click="selectMedia(media)"
        >
          <span class="format-icon">
            <Headphones v-if="media === 'audio'" :size="23" />
            <ImageIcon v-else-if="media === 'comic'" :size="23" />
            <BookOpen v-else :size="23" />
          </span>
          <span class="format-copy">
            <strong>{{ mediaTitle(media) }}</strong>
            <small>{{ mediaDescription(media) }}</small>
          </span>
          <ChevronRight :size="19" />
        </button>
      </section>
      <section
        v-if="
          remoteStatusLabel ||
          novel.typeName ||
          novel.bookshelfName ||
          novel.tags?.length
        "
        class="mobile-book-tags"
        aria-label="作品标签"
      >
        <span
          v-if="remoteStatusLabel"
          class="status-tag"
          :class="{ finished: novel.isFinish }"
          >{{ remoteStatusLabel }}</span
        ><span
          v-if="novel.typeName || novel.bookshelfName"
          class="category-tag"
          >{{ novel.typeName || novel.bookshelfName }}</span
        ><span v-for="tag in novel.tags || []" :key="tag">{{ tag }}</span>
      </section>
      <section class="book-intro" aria-labelledby="book-intro-title">
        <div class="section-title">
          <span />
          <h3 id="book-intro-title">内容简介</h3>
        </div>
        <p :class="{ expanded: descriptionExpanded }">
          {{ novel.description || "暂无简介" }}
        </p>
        <button
          v-if="novel.description && novel.description.length > 104"
          class="expand-description"
          @click="descriptionExpanded = !descriptionExpanded"
        >
          {{ descriptionExpanded ? "收起简介" : "展开简介" }}
        </button>
      </section>
      <button class="mobile-latest-chapter" @click="showDirectory">
        <span class="latest-badge">最近</span>
        <span class="latest-copy">
          <strong>{{ novel.latestChapterTitle || "暂无最新章节信息" }}</strong>
          <small>{{
            formatSourceDate(novel.latestChapterTime || novel.lastUpdateTime)
          }}</small>
        </span>
        <ChevronRight :size="17" />
      </button>
    </section>
    <section ref="directoryElement" class="remote-lower-content">
      <section class="book-discussion" aria-label="读者反馈">
        <DiscussionDanmaku :items="discussionHighlights" />
        <header class="discussion-heading">
          <DiscussionTabs
            :model-value="discussionTab"
            @update:model-value="selectDiscussionTab"
          />
          <button class="all-comments" @click="openComments">
            全部评论<ChevronRight :size="15" />
          </button>
        </header>

        <div v-if="discussion.loading" class="discussion-state">
          <LoaderCircle class="spin" :size="22" />
          {{ discussionTab === "reviews" ? "正在读取书评" : "正在读取评论" }}
        </div>
        <div
          v-else-if="discussion.error && !discussion.items.length"
          class="discussion-state error"
        >
          <span>{{ discussion.error }}</span>
          <button @click="reloadDiscussion">重试</button>
        </div>
        <div v-else-if="!discussion.items.length" class="discussion-state">
          <MessageCircle :size="22" />
          {{ discussionTab === "reviews" ? "暂无书评" : "暂无评论" }}
        </div>
        <div v-else class="discussion-preview">
          <BookCommentCard
            v-for="comment in discussionPreview"
            :key="comment.commentId"
            :comment="comment"
            interactive
            compact
            @select="openComment"
          />
        </div>
      </section>
      <section
        class="remote-directory-panel glass"
        aria-labelledby="remote-directory-title"
      >
        <header class="remote-panel-heading">
          <div>
            <small>REMOTE DIRECTORY</small>
            <h3 id="remote-directory-title">{{ currentMediaLabel }}目录</h3>
          </div>
          <span v-if="currentDirectoryCount"
            >{{ currentDirectoryCount }} 章</span
          >
        </header>
        <div v-if="directoryLoading" class="remote-directory-empty">
          <LoaderCircle class="spin" :size="22" /><span
            >正在读取{{ currentMediaLabel }}目录</span
          >
        </div>
        <div v-else-if="directoryError" class="remote-directory-empty error">
          <span>{{ directoryError }}</span>
          <div class="directory-error-actions">
            <button @click="ensureDirectory(true)">重试</button>
          </div>
        </div>
        <div
          v-else-if="currentDirectoryCount"
          :ref="setDesktopDirectoryElement"
          class="remote-directory-list"
          @scroll.passive="handleDesktopDirectoryScroll"
        >
          <div
            :style="{ height: `${desktopDirectoryWindow.top}px` }"
            aria-hidden="true"
          ></div>
          <template v-for="row in desktopDirectoryWindow.rows" :key="row.key">
            <div
              v-if="row.kind === 'gap'"
              :style="{ height: `${row.height}px` }"
              aria-hidden="true"
            ></div>
            <h4 v-else-if="row.kind === 'volume'" class="remote-volume-heading">
              {{ row.title }}
            </h4>
            <div v-else class="remote-volume-chapters">
              <button
                v-for="chapter in row.chapters"
                :key="chapter.id"
                class="remote-chapter-row"
                @click="startOnline(chapter.id)"
              >
                <strong>{{ chapter.title }}</strong>
                <span
                  v-if="chapter.isVip"
                  class="chapter-badge vip"
                  :class="{ unlocked: chapter.isUnlocked === true }"
                  :title="chapter.isUnlocked === true ? '已解锁' : 'VIP 章节'"
                  ><LockKeyholeOpen
                    v-if="chapter.isUnlocked === true"
                    :size="12"
                    aria-label="已解锁"
                  />VIP</span
                >
                <Play :size="13" />
              </button>
            </div>
          </template>
          <div
            :style="{ height: `${desktopDirectoryWindow.bottom}px` }"
            aria-hidden="true"
          ></div>
        </div>
        <div v-else class="remote-directory-empty">
          <ListTree :size="22" /><span>暂无目录内容</span>
        </div>
      </section>
    </section>
    <nav class="mobile-reading-bar" aria-label="远程作品操作">
      <button
        class="directory-action"
        title="浏览远程目录"
        @click="showDirectory"
      >
        <ListTree :size="21" /><span>目录</span></button
      ><button class="online-action" title="下载章节" @click="openDownload">
        <Download :size="20" /><span>下载</span></button
      ><button
        class="favorite-action"
        :class="{ favorited: isFavorite }"
        :disabled="favoriteBusy"
        :aria-label="isFavorite ? '取消收藏' : '收藏'"
        @click="favoriteWork"
      >
        <LoaderCircle v-if="favoriteBusy" class="spin" :size="18" />
        <Bookmark
          v-else
          :size="18"
          :fill="isFavorite ? 'currentColor' : 'none'"
        /><span>{{ isFavorite ? "已收藏" : "收藏" }}</span></button
      ><button class="primary-action" @click="startOnline()">
        <Play :size="17" />{{ onlineActionLabel }}
      </button>
    </nav>
    <ModalShell
      :open="directoryOpen"
      align="bottom"
      @close="directoryOpen = false"
    >
      <section
        class="directory-drawer"
        role="dialog"
        aria-modal="true"
        aria-labelledby="remote-directory-drawer-title"
      >
        <div class="drawer-handle" aria-hidden="true" />
        <header class="drawer-heading">
          <div>
            <h3 id="remote-directory-drawer-title">
              {{ currentMediaLabel }}目录
            </h3>
          </div>
          <button title="关闭目录" @click="directoryOpen = false">
            <X :size="20" />
          </button>
        </header>
        <div v-if="directoryLoading" class="drawer-empty">
          <LoaderCircle class="spin" :size="27" />正在读取目录
        </div>
        <div v-else-if="directoryError" class="drawer-empty error">
          <span>{{ directoryError }}</span>
          <div class="directory-error-actions">
            <button @click="ensureDirectory(true)">重试</button>
          </div>
        </div>
        <div
          v-else-if="currentDirectoryCount"
          :ref="setDrawerDirectoryElement"
          class="drawer-list"
          @scroll.passive="handleDrawerDirectoryScroll"
        >
          <div
            :style="{ height: `${drawerDirectoryWindow.top}px` }"
            aria-hidden="true"
          ></div>
          <template v-for="row in drawerDirectoryWindow.rows" :key="row.key">
            <div
              v-if="row.kind === 'gap'"
              class="remote-volume-gap"
              :style="{ height: `${row.height}px` }"
              aria-hidden="true"
            ></div>
            <h4 v-else-if="row.kind === 'volume'">{{ row.title }}</h4>
            <button
              v-else
              class="remote-drawer-row"
              @click="startOnline(row.id)"
            >
              <span
                v-if="row.index !== undefined"
                class="remote-drawer-index"
                >{{ String(row.index + 1).padStart(3, "0") }}</span
              >
              <strong>{{ row.title }}</strong>
              <span
                v-if="row.isVip"
                class="chapter-badge vip"
                :class="{ unlocked: row.isUnlocked === true }"
                :title="row.isUnlocked === true ? '已解锁' : 'VIP 章节'"
                ><LockKeyholeOpen
                  v-if="row.isUnlocked === true"
                  :size="12"
                  aria-label="已解锁"
                />VIP</span
              >
              <Play :size="16" />
            </button>
          </template>
          <div
            :style="{ height: `${drawerDirectoryWindow.bottom}px` }"
            aria-hidden="true"
          ></div>
        </div>
        <div v-else class="drawer-empty">
          <ListTree :size="27" />暂无目录内容
        </div>
      </section>
    </ModalShell>
  </section>
  <main v-else class="detail-page detail-state">
    <header class="detail-navigation">
      <button class="nav-circle" title="返回上一页" @click="goBack">
        <ChevronLeft :size="22" /><span class="desktop-nav-label">返回</span>
      </button>
    </header>
    <div class="detail-loading">
      {{ remote.remoteBookLoading.value ? "正在读取作品详情" : "未找到远程作品" }}
    </div>
  </main>
</template>

<style scoped>
.detail-page {
  height: 100%;
  min-height: 100%;
  overflow-y: auto;
  padding-bottom: 34px;
  color: #483f39;
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
.nav-circle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 0;
  font: inherit;
}
.local-label {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
.hero-copy h2,
.desktop-book-copy h2,
.section-title h3 {
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
}
.detail-state {
  display: flex;
  flex-direction: column;
}
.detail-loading {
  display: grid;
  min-height: 0;
  flex: 1;
  color: #a27a6f;
  place-items: center;
}
.desktop-content-tabs {
  display: none;
}
.desktop-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: 1px solid #eaded2;
  border-radius: 9px;
  color: #806b60;
  background: #fffaf5;
  font: inherit;
  font-size: 12px;
}
.desktop-action:hover {
  border-color: #e47552;
  color: #c35e40;
  background: #fff2e9;
}
.desktop-action {
  min-height: 43px;
  padding: 0 15px;
}
.remote-lower-content {
  display: flex;
  width: min(1180px, calc(100% - 64px));
  flex-direction: column;
  gap: 16px;
  margin: 0 auto 42px;
}
.book-discussion {
  min-width: 0;
  padding: 2px 0 6px;
}
.discussion-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 8px 0;
}
.all-comments {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 4px 0 4px 8px;
  border: 0;
  color: #9c7464;
  background: transparent;
  font: inherit;
  font-size: 13px;
  transition: color 160ms ease;
}
.all-comments:hover {
  color: #b55437;
}
.discussion-preview {
  display: grid;
  min-height: 228px;
  gap: 10px;
  padding-top: 4px;
}
.discussion-state {
  display: flex;
  min-height: 228px;
  align-items: center;
  justify-content: center;
  gap: 8px;
  border-top: 1px solid #eee5d9;
  color: #a17d70;
  font-size: 13px;
}
.discussion-state.error {
  flex-direction: column;
  color: #ad6654;
  text-align: center;
}
.discussion-state button {
  min-height: 32px;
  padding: 0 12px;
  border: 1px solid #e7c6b5;
  border-radius: 8px;
  color: #a75b40;
  background: rgba(255, 250, 246, 0.78);
  font: inherit;
  font-size: 12px;
}
.remote-directory-panel {
  min-width: 0;
  border-radius: 17px;
}
.remote-directory-panel {
  display: flex;
  min-height: 280px;
  flex: 1 1 auto;
  flex-direction: column;
  overflow: hidden;
  padding: 20px 0 0;
}
.remote-panel-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 0 20px 14px;
  border-bottom: 1px solid #eee5d9;
}
.remote-panel-heading small {
  color: #b59885;
  font: 10px monospace;
  letter-spacing: 0.08em;
}
.remote-panel-heading h3 {
  margin: 4px 0 0;
  color: #4c4037;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 20px;
}
.remote-panel-heading > span {
  padding: 5px 8px;
  border-radius: 5px;
  color: #b8664e;
  background: #fff0e7;
  font-size: 11px;
}
.latest-badge {
  flex: 0 0 auto;
  padding: 6px 8px;
  border-radius: 6px;
  color: #fff;
  background: #e86e4c;
  font-size: 10px;
}
.remote-directory-list {
  max-height: 500px;
  overflow-y: auto;
  padding: 0 20px 16px;
}
.remote-volume-heading {
  display: block;
  height: 40px;
  margin-bottom: 8px;
  padding: 0;
  overflow: hidden;
  color: #9b7b6c;
  border-bottom: 1px solid #eee5d9;
  font-size: 12px;
  font-weight: 600;
  line-height: 40px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.remote-volume-chapters {
  display: grid;
  height: 41px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 7px 12px;
  align-content: start;
}
.remote-chapter-row {
  display: flex;
  height: 34px;
  min-width: 0;
  min-height: 34px;
  align-items: center;
  gap: 6px;
  padding: 5px 8px;
  border: 1px solid #f0e6dd;
  border-radius: 7px;
  color: #6d5c52;
  background: rgba(255, 253, 248, 0.58);
  font: inherit;
  text-align: left;
  cursor: pointer;
  transition: 0.18s;
}
.remote-chapter-row:hover,
.remote-chapter-row:focus-visible {
  border-color: #e9b797;
  color: #b95438;
  background: #fff3e9;
  outline: none;
}
.remote-chapter-row strong {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  font-size: 12px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chapter-badge {
  display: inline-flex;
  min-height: 20px;
  align-items: center;
  flex: 0 0 auto;
  gap: 3px;
  padding: 0 5px;
  border-radius: 4px;
  color: #887d70;
  background: #f2eee7;
  font-size: 12px;
  white-space: nowrap;
}
.chapter-badge.vip {
  color: #a45b3f;
  background: #fbe2d4;
}
.chapter-badge.vip.unlocked {
  color: #4f8068;
  background: #e7f1ea;
}
.remote-directory-empty {
  display: flex;
  min-height: 112px;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: #aa988b;
  font-size: 12px;
}
.remote-directory-empty.error {
  flex-direction: column;
  color: #b96e5b;
}
.directory-error-actions {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}
.directory-error-actions button {
  min-height: 30px;
  padding: 0 11px;
  border: 1px solid #e8c9bc;
  border-radius: 7px;
  color: #a95843;
  background: #fff7f2;
  font: inherit;
  font-size: 11px;
}
.directory-error-actions button:hover {
  border-color: #d98568;
  background: #fff0e9;
}
.mobile-latest-chapter {
  display: none;
}
.remote-drawer-row {
  display: flex;
  width: 100%;
  height: 48px;
  min-height: 48px;
  align-items: center;
  gap: 8px;
  padding: 0 20px;
  border: none;
  border-top: 1px solid #f0ebdf;
  color: #504a41;
  background: transparent;
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.remote-drawer-row:hover,
.remote-drawer-row:focus-visible {
  color: #b95438;
  background: #fff4ea;
  outline: none;
}
.remote-drawer-row strong {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  font-size: 13px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.remote-drawer-row > svg {
  flex: 0 0 auto;
  margin-left: auto;
  color: #a99f90;
}
.remote-drawer-index {
  flex: 0 0 38px;
  color: #ad9f8d;
  font: 11px monospace;
}
.drawer-empty.error {
  color: #b96e5b;
}
@media (min-width: 901px) {
  .detail-page {
    min-height: 100dvh;
  }
  .book-discussion {
    padding: 20px 22px 12px;
    border: 1px solid rgba(255, 255, 255, 0.8);
    border-radius: 17px;
    background: rgba(255, 255, 255, 0.55);
    box-shadow: 0 15px 38px rgba(137, 76, 55, 0.12);
    backdrop-filter: blur(19px);
    -webkit-backdrop-filter: blur(19px);
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
  .desktop-nav-label {
    font-size: 12px;
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
    margin: 0 auto 21px;
  }
  .desktop-content-tabs {
    display: flex;
    width: min(1180px, calc(100% - 64px));
    gap: 12px;
    margin: 0 auto 18px;
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
    color: #4b3e35;
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
  }
  .desktop-content-tab:disabled {
    cursor: not-allowed;
    opacity: 0.62;
  }
  .desktop-cover-card {
    padding: 14px;
    border-radius: 20px;
  }
  .desktop-cover {
    display: flex;
    width: 100%;
    height: 100%;
    min-height: 390px;
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
    overflow: hidden;
    border-radius: 20px;
  }
  .desktop-book-overview:after {
    position: absolute;
    top: -120px;
    right: -90px;
    width: 260px;
    height: 260px;
    border: 1px solid rgba(226, 148, 100, 0.28);
    border-radius: 50%;
    box-shadow:
      0 0 0 35px rgba(226, 148, 100, 0.06),
      0 0 0 70px rgba(226, 148, 100, 0.035);
    content: "";
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
    flex: 1 1 auto;
    overflow: hidden;
    margin: 11px 0 0;
    padding: 0;
    border: 0;
    color: #766960;
    background: transparent;
    font: inherit;
    font-size: 13px;
    line-height: 1.9;
    text-align: left;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 6;
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
    /* margin-top: auto; */
    padding-top: 11px;
  }
  .desktop-start-reading {
    display: inline-flex;
    min-height: 43px;
    align-items: center;
    justify-content: center;
    gap: 7px;
    min-width: 132px;
    padding: 0 16px;
    border: 0;
    border-radius: 10px;
    color: #fff;
    background: #e47552;
    box-shadow: 0 7px 15px rgba(206, 95, 60, 0.23);
    font: inherit;
    font-size: 13px;
  }
  .desktop-start-reading:hover {
    background: #d96746;
  }
  .desktop-favorite {
    display: inline-flex;
    min-height: 43px;
    align-items: center;
    justify-content: center;
    gap: 7px;
    padding: 0 16px;
    border: 1px solid #e9ad1e;
    border-radius: 10px;
    color: white;
    background: #fcbd30;
    box-shadow: 0 6px 14px rgba(220, 158, 16, 0.2);
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    transition:
      background-color 160ms ease,
      border-color 160ms ease,
      color 160ms ease,
      box-shadow 160ms ease;
  }
  .desktop-favorite:hover:not(:disabled):not(.favorited) {
    border-color: #daa018;
    background: #f2b220;
  }
  .desktop-favorite.favorited {
    border-color: #e2e2e2;
    color: #8d8d8d;
    background: #f0f0f0;
    box-shadow: none;
  }
  .desktop-favorite:disabled {
    cursor: default;
  }
  .desktop-secondary-actions {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-top: 14px;
    color: #907365;
    font-size: 12px;
  }
  .desktop-book-facts {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 14px;
    padding: 14px 14px 0;
    border-radius: 20px;
  }
  .desktop-score-card {
    position: relative;
    display: flex;
    min-height: 112px;
    align-items: end;
    justify-content: space-between;
    padding: 17px;
    overflow: hidden;
    border-radius: 15px;
    color: #fff;
    background: #e29464;
    box-shadow: 0 9px 18px rgba(201, 110, 67, 0.2);
  }
  .desktop-score-card:after {
    position: absolute;
    top: -15px;
    right: -30px;
    font-size: 120px;
    content: "✦";
    opacity: 0.16;
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
  .book-detail-sheet,
  .mobile-reading-bar {
    display: none;
  }
}
@media (max-width: 900px) {
  .detail-page {
    width: 100%;
    margin: 0;
    min-height: 100dvh;
    padding-bottom: calc(34px + env(safe-area-inset-bottom));
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
    padding: 14px 16px;
  }
  .nav-circle {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    color: #fff;
    background: rgba(32, 29, 24, 0.48);
  }
  .desktop-nav-label {
    display: none;
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
    border: 1px solid rgba(255, 255, 255, 0.7);
    border-radius: 14px;
    color: #4f4940;
    box-shadow: 0 10px 22px rgba(85, 73, 53, 0.1);
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
  .format-switch > svg {
    width: 16px;
    height: 16px;
    flex: 0 0 auto;
  }
  .audio-switch {
    color: #60471c;
    background: rgba(242, 194, 72, 0.46);
  }
  .audio-switch .format-icon {
    color: #79470a;
    background: rgba(255, 244, 205, 0.65);
  }
  .novel-switch {
    color: #245e7d;
    background: rgba(167, 215, 239, 0.52);
  }
  .novel-switch .format-icon {
    color: #236c91;
    background: rgba(239, 250, 255, 0.72);
  }
  .comic-switch {
    color: #433860;
    background: rgba(184, 162, 221, 0.42);
  }
  .comic-switch .format-icon {
    color: #604a91;
    background: rgba(246, 240, 255, 0.64);
  }
  .remote-lower-content {
    display: block;
    width: 100%;
    margin: 0;
    padding: 0 20px calc(40px + env(safe-area-inset-bottom));
  }
  .remote-directory-panel {
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
    background: #fff;
    box-shadow: none;
    backdrop-filter: none;
  }
  .book-discussion {
    padding-bottom: 0;
  }
  .discussion-heading {
    padding: 2px 0 8px;
  }
  .discussion-preview,
  .discussion-state {
    min-height: 218px;
    border-top-color: #f0f0f0;
  }
  .discussion-preview {
    gap: 0;
  }
  .discussion-preview :deep(.book-comment) {
    border-bottom: 1px solid #f0f0f0;
  }
  .discussion-preview :deep(.book-comment:last-child) {
    border-bottom: 0;
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
    background: #f0f0f0;
  }
  .mobile-book-tags .category-tag {
    color: #483f39;
    background: #f4f4f4;
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
    font: inherit;
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
  .online-action {
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
  .favorite-action {
    display: inline-flex;
    min-height: 48px;
    flex: 1 1 0;
    min-width: 0;
    align-items: center;
    justify-content: center;
    gap: 5px;
    padding: 0 11px;
    border: 0;
    border-radius: 999px;
    color: white;
    background: linear-gradient(90deg, #fcbd30 0%, #fdab31 100%);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    transition:
      background-color 160ms ease,
      border-color 160ms ease,
      color 160ms ease;
  }
  .favorite-action.favorited {
    color: #8d8d8d;
    background: #f0f0f0;
  }
  .favorite-action:disabled {
    cursor: default;
  }
  .primary-action {
    display: inline-flex;
    min-height: 48px;
    flex: 1 1 0;
    min-width: 0;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: 0;
    border-radius: 999px;
    color: #fff;
    background: #eb6545;
    box-shadow: 0 5px 13px rgba(207, 91, 62, 0.25);
    font: inherit;
    font-size: 14px;
    font-weight: 700;
  }
}

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
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 22px;
}
.description-modal header button {
  display: inline-flex;
  width: 34px;
  height: 34px;
  flex: 0 0 auto;
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
.directory-drawer {
  display: flex;
  width: min(100%, 720px);
  max-height: min(82dvh, 720px);
  flex-direction: column;
  overflow: hidden;
  padding: 8px 0 0;
  border-radius: 24px 24px 0 0;
  background: #fffdf8;
  box-shadow: 0 -20px 48px rgba(46, 40, 33, 0.26);
}
.drawer-handle {
  width: 42px;
  height: 5px;
  margin: 8px auto 7px;
  border-radius: 3px;
  background: #d2c9b9;
}
.drawer-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 9px 20px 16px;
  border-bottom: 1px solid #eee8dc;
}
.drawer-heading small {
  color: #b59885;
  font: 10px monospace;
  letter-spacing: 0.08em;
}
.drawer-heading h3 {
  margin: 4px 0 0;
  color: #443e36;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 22px;
}
.drawer-heading button {
  display: inline-flex;
  width: 38px;
  height: 38px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 50%;
  color: #79695e;
  background: #f4eee4;
}
.drawer-list {
  overflow-y: auto;
  padding-bottom: env(safe-area-inset-bottom);
  background: #fffdf8;
}
.remote-volume-gap {
  background: #f7f3ea;
}
.drawer-list h4 {
  display: block;
  height: 42px;
  margin: 0;
  padding: 0 20px;
  overflow: hidden;
  color: #887f70;
  font-size: 12px;
  line-height: 42px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.drawer-empty {
  display: flex;
  min-height: 160px;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: #a0998d;
  font-size: 13px;
}
@media (prefers-reduced-motion: reduce) {
  .desktop-favorite,
  .favorite-action {
    transition: none;
  }
}
</style>
