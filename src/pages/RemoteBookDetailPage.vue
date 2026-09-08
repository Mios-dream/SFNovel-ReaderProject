<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { computed, inject, nextTick, ref, watch } from "vue";
import {
  BookOpen,
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
  Star,
  Ticket,
  X,
} from "lucide-vue-next";
import { deskInjectionKey } from "../deskContext";
import type { Chapter, ChapterMode, ChapterVolume } from "../types";

function requireDesk() {
  const value = inject(deskInjectionKey);
  if (!value) throw new Error("Desk context is unavailable");
  return value;
}

const desk = requireDesk();
const novel = computed(() => desk.remoteBook.value);
type RemoteMediaType = "novel" | "audio" | "comic";
const mediaTypes: RemoteMediaType[] = ["novel", "audio", "comic"];
const descriptionExpanded = ref(false);
const descriptionModalOpen = ref(false);
const mode = computed<ChapterMode>(() =>
  novel.value?.bookshelfType === "audio"
    ? "audio"
    : novel.value?.bookshelfType === "comic"
      ? "comic"
      : "text",
);
const remoteMedia = computed(() => desk.remoteMedia.value);
const remoteMediaDetecting = computed(() => desk.remoteMediaDetecting.value);
const mobileSwitchMediaTypes = computed(() =>
  mediaTypes.filter((media) => media !== selectedMedia.value),
);
const directoryOpen = ref(false);
const directoryElement = ref<HTMLElement>();
const directoryLoading = ref(false);
const directoryError = ref("");
const loadedDirectoryKey = ref("");
const textVolumes = ref<ChapterVolume[]>([]);
const audioChapters = ref<Chapter[]>([]);
const comicChapters = ref<Chapter[]>([]);
const audioVolumes = computed(() => groupChaptersByVolume(audioChapters.value));
const comicVolumes = computed(() => groupChaptersByVolume(comicChapters.value));
const selectedMedia = computed<RemoteMediaType>(
  () => novel.value?.bookshelfType || "novel",
);
const currentDirectoryCount = computed(() =>
  mode.value === "text"
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
const remoteStatusLabel = computed(() =>
  novel.value?.isFinish == null
    ? ""
    : novel.value.isFinish
      ? "已完结"
      : "连载中",
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
  desk.navigate("bookshelf");
}
function openDownload() {
  if (novel.value) void desk.openChapterPicker(novel.value, mode.value);
}

function selectMedia(media: RemoteMediaType) {
  if (remoteMedia.value[media]) desk.selectRemoteMedia(media);
}

function mediaTitle(media: RemoteMediaType) {
  return media === "audio"
    ? "有声小说"
    : media === "comic"
      ? "漫画改编"
      : "小说原著";
}

function mediaDescription(media: RemoteMediaType) {
  if (remoteMedia.value[media]) {
    return media === selectedMedia.value ? "当前查看" : "可切换查看";
  }
  return remoteMediaDetecting.value ? "正在探测" : "暂未发现";
}

function directoryKey() {
  const work = novel.value;
  return work ? `${mode.value}:${work.novelId}:${work.sourcePath || ""}` : "";
}

function clearDirectory() {
  directoryLoading.value = false;
  directoryError.value = "";
  loadedDirectoryKey.value = "";
  textVolumes.value = [];
  audioChapters.value = [];
  comicChapters.value = [];
}

async function browseDirectory() {
  const work = novel.value;
  if (!work) return;
  const isMobile = window.matchMedia("(max-width: 900px)").matches;
  directoryOpen.value = isMobile;
  const key = directoryKey();
  if (key !== loadedDirectoryKey.value) {
    directoryLoading.value = true;
    directoryError.value = "";
    textVolumes.value = [];
    audioChapters.value = [];
    comicChapters.value = [];
    try {
      if (mode.value === "text") {
        textVolumes.value = await invoke<ChapterVolume[]>(
          "get_chapter_volumes",
          { novelId: work.novelId },
        );
      } else if (mode.value === "audio") {
        if (!desk.auth.value.webAuthenticated) {
          desk.credentialsOpen.value = true;
          throw new Error("连接网站登录后可读取有声目录");
        }
        const catalog = await invoke<{ chapters: Chapter[] }>(
          "get_audio_chapters",
          { novelId: work.novelId },
        );
        audioChapters.value = catalog.chapters;
      } else {
        const catalog = await invoke<{ chapters: Chapter[] }>(
          "get_comic_chapters",
          {
            comicId: work.novelId,
            sourcePath: work.sourcePath,
            titleHint: work.novelName,
          },
        );
        comicChapters.value = catalog.chapters;
      }
      loadedDirectoryKey.value = key;
    } catch (error) {
      directoryError.value =
        error instanceof Error ? error.message : "读取章节目录失败";
    } finally {
      directoryLoading.value = false;
    }
  }
  await nextTick();
  if (!isMobile) {
    directoryElement.value?.scrollIntoView({
      behavior: "smooth",
      block: "nearest",
    });
  }
}

async function openOnline() {
  const work = novel.value;
  if (!work) return;
  const url =
    work.bookshelfType === "comic" && work.sourcePath
      ? `https://manhua.sfacg.com/mh/${work.sourcePath}/`
      : `https://book.sfacg.com/Novel/${work.novelId}/`;
  try {
    await openUrl(url);
  } catch {
    // The opener plugin reports platform-specific failures without a safe detail.
  }
}

watch(
  () => novel.value,
  () => {
    descriptionExpanded.value = false;
    descriptionModalOpen.value = false;
    directoryOpen.value = false;
    clearDirectory();
  },
);
function formatMetric(value?: number) {
  if (typeof value !== "number" || !Number.isFinite(value)) return "--";
  if (Math.abs(value) >= 100_000_000)
    return `${trimMetric(value / 100_000_000)}亿`;
  if (Math.abs(value) >= 10_000) return `${trimMetric(value / 10_000)}万`;
  return Math.round(value).toLocaleString("zh-CN");
}
function trimMetric(value: number) {
  return value >= 100 ? value.toFixed(0) : value.toFixed(1).replace(/\.0$/, "");
}
function scoreToneClass(value?: number) {
  return typeof value !== "number"
    ? "score-unknown"
    : value >= 9
      ? "score-excellent"
      : value >= 8
        ? "score-good"
        : value >= 6
          ? "score-average"
          : "score-low";
}
function formatSourceDate(value?: string) {
  if (!value) return "--";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleDateString("zh-CN");
}
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

function textChapterBadge(kind: ChapterVolume["chapters"][number]["contentKind"]) {
  if (kind === "imageVip") return "图片 OCR";
  if (kind === "encryptedVip") return "加密 OCR";
  return "VIP";
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
        <button class="nav-circle" title="返回远程书架" @click="goBack">
          <ChevronLeft :size="22" /><span class="desktop-nav-label"
            >返回书架</span
          >
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
            ><span v-if="novel.typeName || novel.bookshelfName"
              >分类 · {{ novel.typeName || novel.bookshelfName }}</span
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
            <button class="desktop-start-reading" @click="openDownload">
              <Download :size="18" />下载
            </button>
            <button class="desktop-action" @click="openOnline">
              <ExternalLink :size="17" />在线查看
            </button>
            <button class="desktop-action" @click="browseDirectory">
              <ListTree :size="17" />浏览目录
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

    <div
      v-if="descriptionModalOpen"
      class="description-modal-backdrop"
      @click.self="descriptionModalOpen = false"
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
    </div>

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
        ><span v-if="novel.typeName || novel.bookshelfName" class="category-tag"
          >分类 · {{ novel.typeName || novel.bookshelfName }}</span
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
      <button class="mobile-latest-chapter" @click="browseDirectory">
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
      <section
        class="remote-directory-panel glass"
        aria-labelledby="remote-directory-title"
      >
        <header class="remote-panel-heading">
          <div>
            <small>REMOTE DIRECTORY</small>
            <h3 id="remote-directory-title">{{ currentMediaLabel }}目录</h3>
          </div>
          <button
            class="directory-load-button"
            :disabled="directoryLoading"
            @click="browseDirectory"
          >
            <LoaderCircle v-if="directoryLoading" class="spin" :size="16" />
            <ListTree v-else :size="16" />
            {{ directoryLoading ? "加载中" : "加载目录" }}
          </button>
        </header>
        <div v-if="directoryLoading" class="remote-directory-empty">
          <LoaderCircle class="spin" :size="22" /><span
            >正在读取{{ currentMediaLabel }}目录</span
          >
        </div>
        <div v-else-if="directoryError" class="remote-directory-empty error">
          <span>{{ directoryError }}</span>
        </div>
        <div v-else-if="currentDirectoryCount" class="remote-directory-list">
          <template v-if="mode === 'text'">
            <section
              v-for="volume in textVolumes"
              :key="volume.volumeId"
              class="remote-volume"
            >
              <h4>{{ volume.title }}</h4>
              <div class="remote-volume-chapters">
                <div
                  v-for="chapter in volume.chapters"
                  :key="chapter.chapId"
                  class="remote-chapter-row"
                >
                  <strong>{{ chapter.title }}</strong>
                  <span v-if="chapter.isVip" class="chapter-badge vip">{{
                    textChapterBadge(chapter.contentKind)
                  }}</span>
                </div>
              </div>
            </section>
          </template>
          <template v-else>
            <section
              v-for="volume in mode === 'audio' ? audioVolumes : comicVolumes"
              :key="volume.title"
              class="remote-volume"
            >
              <h4>{{ volume.title }}</h4>
              <div class="remote-volume-chapters">
                <div
                  v-for="chapter in volume.chapters"
                  :key="chapter.id"
                  class="remote-chapter-row"
                >
                  <strong>{{ chapter.title }}</strong>
                  <span v-if="chapter.isVip" class="chapter-badge vip"
                    >VIP</span
                  >
                </div>
              </div>
            </section>
          </template>
        </div>
        <div v-else class="remote-directory-empty">
          <ListTree :size="22" /><span>点击加载目录后在此浏览章节</span>
        </div>
      </section>
    </section>
    <nav class="mobile-reading-bar" aria-label="远程作品操作">
      <button
        class="directory-action"
        title="浏览远程目录"
        @click="browseDirectory"
      >
        <ListTree :size="21" /><span>目录</span></button
      ><button class="online-action" title="在线查看" @click="openOnline">
        <ExternalLink :size="20" /><span>在线</span></button
      ><button class="primary-action" @click="openDownload">
        <Download :size="17" />下载
      </button>
    </nav>
    <div
      v-if="directoryOpen"
      class="directory-backdrop"
      @click.self="directoryOpen = false"
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
            <small>远程内容</small>
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
          {{ directoryError }}
        </div>
        <div
          v-else-if="currentDirectoryCount && mode === 'text'"
          class="drawer-list"
        >
          <section
            v-for="volume in textVolumes"
            :key="volume.volumeId"
            class="chapter-volume"
          >
            <h4>{{ volume.title }}</h4>
            <div
              v-for="chapter in volume.chapters"
              :key="chapter.chapId"
              class="remote-drawer-row"
            >
              <strong>{{ chapter.title }}</strong>
              <span v-if="chapter.isVip" class="chapter-badge vip">{{
                textChapterBadge(chapter.contentKind)
              }}</span>
              <ChevronRight :size="16" />
            </div>
          </section>
        </div>
        <div v-else-if="currentDirectoryCount" class="drawer-list">
          <div
            v-for="(chapter, index) in mode === 'audio'
              ? audioChapters
              : comicChapters"
            :key="chapter.id"
            class="remote-drawer-row"
          >
            <span v-if="mode === 'audio'" class="remote-drawer-index">{{
              String(index + 1).padStart(3, "0")
            }}</span>
            <strong>{{ chapter.title }}</strong>
            <span v-if="chapter.isVip" class="chapter-badge vip">VIP</span>
            <ChevronRight :size="16" />
          </div>
        </div>
        <div v-else class="drawer-empty">
          <ListTree :size="27" />暂无目录内容
        </div>
      </section>
    </div>
  </section>
  <main v-else class="detail-loading">
    {{ desk.remoteBookLoading.value ? "正在读取作品详情" : "未找到远程作品" }}
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
.detail-loading {
  display: grid;
  min-height: 100dvh;
  color: #a27a6f;
  background: var(--background-color);
  place-items: center;
}
.desktop-content-tabs {
  display: none;
}
.desktop-action,
.directory-load-button {
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
.desktop-action:hover,
.directory-load-button:hover {
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
.remote-latest-panel,
.remote-directory-panel {
  min-width: 0;
  border-radius: 17px;
}
.remote-latest-panel {
  padding: 20px;
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
.directory-load-button {
  min-height: 34px;
  padding: 0 11px;
  border-radius: 8px;
  font-size: 11px;
}
.remote-latest-row {
  display: flex;
  align-items: center;
  gap: 11px;
}
.remote-latest-row > div {
  display: flex;
  min-width: 0;
  flex: 1 1 auto;
  flex-direction: column;
  gap: 7px;
}
.remote-latest-row strong {
  overflow: hidden;
  color: #625149;
  font-size: 13px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.remote-latest-row small {
  color: #a5968b;
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
  display: flex;
  max-height: 500px;
  flex-direction: column;
  overflow-y: auto;
  padding: 0 20px 16px;
}
.remote-volume {
  width: 100%;
  padding: 16px 0 9px;
  border-bottom: 1px solid #eee5d9;
}
.remote-volume h4 {
  margin: 0 0 10px;
  color: #9b7b6c;
  font-size: 12px;
  font-weight: 600;
}
.remote-volume-chapters {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 7px 12px;
}
.remote-chapter-row {
  display: flex;
  min-width: 0;
  min-height: 34px;
  align-items: center;
  gap: 6px;
  padding: 5px 8px;
  border: 1px solid #f0e6dd;
  border-radius: 7px;
  color: #6d5c52;
  background: rgba(255, 253, 248, 0.58);
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
  min-height: 18px;
  align-items: center;
  flex: 0 0 auto;
  padding: 0 5px;
  border-radius: 4px;
  color: #887d70;
  background: #f2eee7;
  font-size: 9px;
  white-space: nowrap;
}
.chapter-badge.vip {
  color: #a45b3f;
  background: #fbe2d4;
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
  color: #b96e5b;
}
.mobile-latest-chapter {
  display: none;
}
.remote-drawer-row {
  display: flex;
  width: 100%;
  min-height: 48px;
  align-items: center;
  gap: 8px;
  padding: 0 20px;
  border-top: 1px solid #f0ebdf;
  color: #504a41;
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
    width: calc(100% + 24px);
    margin: calc(-1 * (8px + env(safe-area-inset-top))) -12px
      calc(-78px - env(safe-area-inset-bottom));
    min-height: 100dvh;
    padding-bottom: calc(34px + env(safe-area-inset-bottom));
    background: var(--background-color);
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
    display: none;
  }
  .book-detail-sheet {
    position: relative;
    z-index: 2;
    display: block;
    width: 100%;
    margin-top: -31px;
    padding: 25px 20px 116px;
    border: 1px solid rgba(255, 255, 255, 0.8);
    border-radius: 28px 28px 0 0;
    background: rgba(255, 255, 255, 0.55);
    box-shadow: 0 15px 38px rgba(137, 76, 55, 0.12);
    backdrop-filter: blur(19px);
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
    gap: 8px;
    padding-top: 17px;
  }
  .mobile-book-tags span {
    display: inline-flex;
    min-height: 27px;
    align-items: center;
    max-width: 100%;
    padding: 0 10px;
    overflow: hidden;
    border: 1px solid #e6dfcf;
    border-radius: 999px;
    color: #746b60;
    background: #f8f5eb;
    font-size: 11px;
    line-height: 1;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mobile-book-tags .status-tag {
    border-color: #e86e4c;
    color: #fff;
    background: #e86e4c;
  }
  .mobile-book-tags .status-tag.finished {
    border-color: #6f9f88;
    background: #6f9f88;
  }
  .mobile-book-tags .category-tag {
    border-color: #d9c5e8;
    color: #806192;
    background: #f5eef9;
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
    min-height: 54px;
    align-items: center;
    gap: 9px;
    margin-top: 15px;
    padding: 0 14px;
    border: 1px solid rgba(255, 255, 255, 0.3);
    border-radius: 16px;
    color: #756c61;
    background: rgba(250, 250, 250, 0.5);
    box-shadow: 0 4px 12px rgba(85, 73, 53, 0.08);
    text-align: left;
  }
  .latest-badge {
    padding: 6px 8px;
    border-radius: 14px;
    color: #fff;
    background: #e86e4c;
    font-size: 10px;
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
    font-size: 12px;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .latest-copy small {
    flex: 0 0 auto;
    color: #a29889;
    font-size: 10px;
  }
  .mobile-latest-chapter > svg {
    flex: 0 0 auto;
    color: #a39a8a;
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
    background: rgba(255, 253, 248, 0.97);
    box-shadow: 0 -7px 22px rgba(58, 48, 38, 0.1);
    backdrop-filter: blur(13px);
  }
  .directory-action {
    display: flex;
    min-height: 48px;
    flex: 0 0 52px;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    padding: 0;
    border: 0;
    color: #625a4e;
    background: transparent;
    font: inherit;
    font-size: 10px;
  }
  .online-action {
    display: flex;
    min-height: 48px;
    flex: 0 0 52px;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    padding: 0;
    border: 0;
    color: #625a4e;
    background: transparent;
    font: inherit;
    font-size: 10px;
  }
  .primary-action {
    display: inline-flex;
    min-height: 48px;
    flex: 1 1 0;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: 0;
    border-radius: 14px;
    color: #fff;
    background: #eb6545;
    box-shadow: 0 5px 13px rgba(207, 91, 62, 0.25);
    font: inherit;
    font-size: 14px;
    font-weight: 700;
  }
}

.description-modal-backdrop {
  position: fixed;
  z-index: 70;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(43, 36, 31, 0.38);
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
.directory-backdrop {
  position: fixed;
  z-index: 50;
  inset: 0;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  padding: 0;
  background: rgba(43, 36, 31, 0.46);
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
.chapter-volume + .chapter-volume {
  border-top: 9px solid #f7f3ea;
}
.chapter-volume h4 {
  margin: 0;
  padding: 12px 20px 9px;
  color: #887f70;
  font-size: 12px;
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
</style>
