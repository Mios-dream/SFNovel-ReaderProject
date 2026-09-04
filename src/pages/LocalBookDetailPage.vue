<script setup lang="ts">
import { computed, inject, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  BookOpen,
  ChevronLeft,
  ChevronRight,
  Download,
  ExternalLink,
  FileDown,
  Headphones,
  Images,
  LibraryBig,
  ListTree,
  Play,
  Disc3,
  Heart,
  Star,
  Ticket,
  X,
} from "lucide-vue-next";
import ExportModal from "../components/ExportModal.vue";
import { deskInjectionKey } from "../deskContext";

function requireDesk() {
  const desk = inject(deskInjectionKey);
  if (!desk) throw new Error("Desk context is unavailable");
  return desk;
}

const desk = requireDesk();
const route = useRoute();
const router = useRouter();
const book = computed(() => desk.localBook.value);
const exporting = desk.exportingFormat;

const textChapterCount = computed(() =>
  (book.value?.chapterVolumes ?? []).reduce(
    (count, volume) => count + volume.chapters.length,
    0,
  ),
);
const audioChapterCount = computed(() => book.value?.audioTracks.length ?? 0);
const comicChapterCount = computed(() => book.value?.comicChapters.length ?? 0);
const firstTextChapterId = computed(
  () => book.value?.chapterVolumes[0]?.chapters[0]?.id,
);
const chapterMode = ref<"text" | "audio" | "comic">(
  textChapterCount.value ? "text" : audioChapterCount.value ? "audio" : "comic",
);
const desktopContentTab = ref<"novel" | "audio" | "comic">(
  textChapterCount.value
    ? "novel"
    : audioChapterCount.value
      ? "audio"
      : comicChapterCount.value ? "comic" : "novel",
);
const desktopDirectoryCount = computed(() =>
  desktopContentTab.value === "novel"
    ? textChapterCount.value
    : desktopContentTab.value === "audio"
      ? audioChapterCount.value
      : comicChapterCount.value,
);
const detailMode = ref<"novel" | "comic">("novel");
const directoryOpen = ref(false);
const exportModalOpen = ref(false);
const descriptionExpanded = ref(false);
const descriptionModalOpen = ref(false);
const desktopDirectoryElement = ref<HTMLElement>();
const primaryActionLabel = computed(() =>
  textChapterCount.value ? "开始阅读" : audioChapterCount.value ? "播放有声" : "阅读漫画",
);
const localStatusLabel = computed(() => {
  if (book.value?.isFinished == null) return "";
  return book.value.isFinished ? "已完结" : "连载中";
});
const desktopBookFacts = computed(() => {
  const currentBook = book.value;
  if (!currentBook) return [];

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

function formatMetric(value?: number) {
  if (typeof value !== "number" || !Number.isFinite(value)) return "--";
  const absolute = Math.abs(value);
  if (absolute >= 100_000_000) {
    return `${trimMetric(value / 100_000_000)}亿`;
  }
  if (absolute >= 10_000) {
    return `${trimMetric(value / 10_000)}万`;
  }
  return Math.round(value).toLocaleString("zh-CN");
}

function trimMetric(value: number) {
  return value >= 100 ? value.toFixed(0) : value.toFixed(1).replace(/\.0$/, "");
}

function scoreToneClass(value?: number) {
  if (typeof value !== "number") return "score-unknown";
  if (value >= 9) return "score-excellent";
  if (value >= 8) return "score-good";
  if (value >= 6) return "score-average";
  return "score-low";
}

function formatSourceDate(value?: string) {
  if (!value) return "--";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleDateString("zh-CN");
}

async function loadRouteBook() {
  const name = route.params.name;
  if (typeof name !== "string" || !name) {
    await router.replace("/library");
    return;
  }
  if (desk.localBook.value?.name !== name) {
    const loadedBook = await desk.loadLocalBook(name);
    if (!loadedBook) {
      await router.replace("/library");
      return;
    }
  }
  chapterMode.value = textChapterCount.value ? "text" : audioChapterCount.value ? "audio" : "comic";
  desktopContentTab.value = textChapterCount.value
    ? "novel"
    : audioChapterCount.value
      ? "audio"
      : comicChapterCount.value ? "comic" : "novel";
  desk.navigate("libraryDetail");
}

async function goBack() {
  const target = desk.backFromLibraryDetail();
  desk.navigate(target);
  await router.push(`/${target}`);
}

async function readChapter(chapterId: number) {
  directoryOpen.value = false;
  await desk.openLocalChapter(chapterId);
  if (desk.localChapter.value) {
    desk.navigate("reader");
    await router.push("/reader");
  }
}

async function playAudio(trackIndex: number) {
  directoryOpen.value = false;
  desk.openLocalAudioPlayer(trackIndex);
  if (desk.localBook.value?.audioTracks.length) {
    desk.navigate("audioPlayer");
    await router.push("/audio");
  }
}

async function readComicChapter(chapterId: number) {
  directoryOpen.value = false;
  await desk.openLocalComicChapter(chapterId);
  if (desk.localComicChapter.value) {
    desk.navigate("reader");
    await router.push("/reader");
  }
}

function startReading() {
  if (firstTextChapterId.value) {
    void readChapter(firstTextChapterId.value);
    return;
  }
  if (audioChapterCount.value) void playAudio(0);
  else if (comicChapterCount.value) void readComicChapter(book.value!.comicChapters[0].id);
}

function openDirectory(mode: "text" | "audio" | "comic" = "text") {
  chapterMode.value =
    mode === "audio" && audioChapterCount.value
      ? "audio"
      : textChapterCount.value
        ? "text"
        : audioChapterCount.value ? "audio" : "comic";
  directoryOpen.value = true;
}

function focusDesktopDirectory(mode: "text" | "audio" | "comic" = "text") {
  if (mode === "audio" && audioChapterCount.value) {
    desktopContentTab.value = "audio";
    chapterMode.value = "audio";
  } else if (textChapterCount.value) {
    desktopContentTab.value = "novel";
    chapterMode.value = "text";
  } else if (audioChapterCount.value) {
    desktopContentTab.value = "audio";
    chapterMode.value = "audio";
  } else if (comicChapterCount.value) {
    desktopContentTab.value = "comic";
    chapterMode.value = "comic";
  } else {
    desktopContentTab.value = "novel";
  }
  desktopDirectoryElement.value?.scrollIntoView({
    behavior: "smooth",
    block: "nearest",
  });
}

function selectDesktopContentTab(tab: "novel" | "audio" | "comic") {
  if (tab === "novel") {
    if (!textChapterCount.value) return;
    chapterMode.value = "text";
  } else if (tab === "audio") {
    if (!audioChapterCount.value) return;
    chapterMode.value = "audio";
  } else if (!comicChapterCount.value) return;
  desktopContentTab.value = tab;
}

function openAudioDetail() {
  if (audioChapterCount.value) void playAudio(0);
}

function openComicDetail() {
  detailMode.value = "comic";
  directoryOpen.value = false;
}

onMounted(() => void loadRouteBook());
watch(
  () => route.params.name,
  () => void loadRouteBook(),
);
</script>

<template>
  <section
    v-if="book"
    class="detail-page"
    :class="{ 'comic-page': detailMode === 'comic' }"
  >
    <section
      v-if="detailMode === 'novel'"
      class="book-hero"
      :class="{ 'without-cover': !book.cover }"
    >
      <div
        v-if="book.cover"
        class="hero-art"
        :style="{ backgroundImage: `url('${book.cover}')` }"
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
        <h2>{{ book.name }}</h2>
        <div class="hero-meta-row">
          <span class="hero-author">{{ book.author || "未知作者" }}</span>
          <span
            v-if="book.score != null"
            class="hero-score"
            :class="scoreToneClass(book.score)"
          >
            <Star :size="14" fill="currentColor" />
            <strong>{{ book.score.toFixed(1) }}</strong>
          </span>
        </div>
      </div>
    </section>

    <section v-if="detailMode === 'novel'" class="desktop-book-workbench">
      <section class="desktop-book-hero-layout">
        <section class="desktop-cover-card glass">
          <div class="desktop-cover" :class="{ 'without-cover': !book.cover }">
            <img
              v-if="book.cover"
              :src="book.cover"
              :alt="`${book.name} 封面`"
            />
            <BookOpen v-else :size="54" />
          </div>
        </section>
        <section class="desktop-book-overview glass">
          <div class="desktop-book-copy">
            <h2>{{ book.name }}</h2>
            <p class="desktop-author">{{ book.author || "未知作者" }}</p>
            <div class="desktop-tags" aria-label="作品标签">
              <span
                v-if="localStatusLabel"
                class="status-tag"
                :class="{ finished: book.isFinished }"
                >{{ localStatusLabel }}</span
              >
              <span v-if="book.typeName">分类 · {{ book.typeName }}</span>
              <span v-for="tag in book.tags" :key="tag">{{ tag }}</span>
            </div>
            <button
              class="desktop-description-preview"
              :disabled="!book.description"
              @click="descriptionModalOpen = true"
            >
              <span> {{ book.description || "暂无本地简介" }}</span>
            </button>

            <div class="desktop-primary-actions">
              <button
                class="desktop-start-reading"
                :disabled="!textChapterCount && !audioChapterCount"
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
              <button v-if="book.novelId" @click="desk.continueDownload">
                <Download :size="16" />补充下载
              </button>
              <button v-if="book.novelId" @click="desk.readOnline">
                <ExternalLink :size="16" />在线查看
              </button>
              <button
                :disabled="
                  Boolean(exporting) ||
                  (!textChapterCount && !audioChapterCount)
                "
                @click="exportModalOpen = true"
              >
                <FileDown :size="16" />{{ exporting ? "正在导出" : "导出内容" }}
              </button>
            </div>
          </div>
        </section>
        <aside class="desktop-book-facts glass" aria-label="作品状态">
          <div
            v-if="book.score != null"
            class="desktop-score-card"
            :class="scoreToneClass(book.score)"
          >
            <div>
              <small>源站评分</small>
              <strong>{{ book.score.toFixed(1) }}</strong>
            </div>
            <span><Star :size="15" fill="currentColor" />仅作参考</span>
          </div>
          <div class="desktop-key-metrics" aria-label="作品核心数据">
            <div>
              <BookOpen :size="16" />
              <strong>{{ formatMetric(book.characterCount) }}</strong>
              <span>字数</span>
            </div>
            <div>
              <Ticket :size="16" />
              <strong>{{ formatMetric(book.ticketCount) }}</strong>
              <span>月票</span>
            </div>
            <div>
              <Heart :size="16" fill="currentColor" />
              <strong>{{ formatMetric(book.markCount) }}</strong>
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
          <span><strong>漫画</strong><small>{{ comicChapterCount ? `${comicChapterCount} 个章节` : '尚无本地漫画章节' }}</small></span>
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
          v-if="desktopContentTab === 'novel' && textChapterCount"
          class="desktop-directory-list"
        >
          <section v-for="volume in book.chapterVolumes" :key="volume.volume">
            <h4>{{ volume.volume }}</h4>
            <button
              v-for="chapter in volume.chapters"
              :key="chapter.id"
              @click="readChapter(chapter.id)"
            >
              <span>{{ chapter.title }}</span
              ><ChevronRight :size="15" />
            </button>
          </section>
        </div>
        <div
          v-else-if="desktopContentTab === 'audio' && audioChapterCount"
          class="desktop-directory-list audio-directory-list"
        >
          <button
            v-for="(track, index) in book.audioTracks"
            :key="track.href"
            @click="playAudio(index)"
          >
            <small>{{ String(index + 1).padStart(3, "0") }}</small
            ><span>{{ track.title }}</span
            ><Play :size="14" fill="currentColor" />
          </button>
        </div>
        <div v-else-if="desktopContentTab === 'comic' && comicChapterCount" class="desktop-directory-list comic-directory-list">
          <button v-for="chapter in book.comicChapters" :key="chapter.id" @click="readComicChapter(chapter.id)">
            <span>{{ chapter.title }}</span><ChevronRight :size="15" />
          </button>
        </div>
        <div v-else class="desktop-directory-empty">
          <Images v-if="desktopContentTab === 'comic'" :size="27" />
          <ListTree v-else :size="27" />
          {{
            desktopContentTab === "comic" ? "暂无漫画章节" : "暂无已下载内容"
          }}
        </div>
      </aside>

      <div
        v-if="descriptionModalOpen"
        class="description-modal-backdrop"
        @click.self="descriptionModalOpen = false"
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
          <p>{{ book.description || "暂无本地简介" }}</p>
        </section>
      </div>
    </section>

    <section v-if="detailMode === 'novel'" class="book-detail-sheet">
      <section class="summary-row" aria-label="作品核心数据">
        <div>
          <BookOpen :size="16" />
          <strong>{{ formatMetric(book.characterCount) }}</strong
          ><span>字数</span>
        </div>
        <div>
          <Ticket :size="16" />
          <strong>{{ formatMetric(book.ticketCount) }}</strong
          ><span>月票</span>
        </div>
        <div>
          <Heart :size="16" fill="currentColor" />
          <strong>{{ formatMetric(book.markCount) }}</strong
          ><span>点赞</span>
        </div>
      </section>

      <section
        v-if="localStatusLabel || book.typeName || book.tags.length"
        class="mobile-book-tags"
        aria-label="作品标签"
      >
        <span
          v-if="localStatusLabel"
          class="status-tag"
          :class="{ finished: book.isFinished }"
          >{{ localStatusLabel }}</span
        >
        <span v-if="book.typeName" class="category-tag"
          >分类 · {{ book.typeName }}</span
        >
        <span v-for="tag in book.tags" :key="tag">{{ tag }}</span>
      </section>

      <section class="format-switches" aria-label="内容类型">
        <button
          class="format-switch audio-switch glass"
          :disabled="!audioChapterCount"
          @click="openAudioDetail"
        >
          <span class="format-icon"><Disc3 :size="23" /></span>
          <span class="format-copy"
            ><strong>有声小说</strong
            ><small>{{
              audioChapterCount
                ? `${audioChapterCount} 条本地音轨`
                : "暂无本地音轨"
            }}</small></span
          >
          <ChevronRight :size="19" />
        </button>
        <button
          class="format-switch comic-switch glass"
          @click="openComicDetail"
        >
          <span class="format-icon"><Images :size="23" /></span>
          <span class="format-copy"
            ><strong>漫画改编</strong><small>探索作品的视觉篇章</small></span
          >
          <ChevronRight :size="19" />
        </button>
      </section>

      <section class="book-intro" aria-labelledby="book-intro-title">
        <div class="section-title">
          <span />
          <h3 id="book-intro-title">内容简介</h3>
        </div>
        <p :class="{ expanded: descriptionExpanded }">
          {{ book.description || "暂无本地简介" }}
        </p>
        <button
          v-if="book.description && book.description.length > 104"
          class="expand-description"
          @click="descriptionExpanded = !descriptionExpanded"
        >
          {{ descriptionExpanded ? "收起简介" : "展开简介" }}
        </button>
      </section>

      <button class="mobile-latest-chapter" @click="openDirectory()">
        <span class="latest-badge">最近</span>
        <span class="latest-copy">
          <strong>{{ book.latestChapterTitle || "暂无最新章节" }}</strong>
          <small>{{
            formatSourceDate(book.latestChapterTime || book.lastUpdateTime)
          }}</small>
        </span>
        <ChevronRight :size="17" />
      </button>

      <section class="support-actions" aria-label="本地作品操作">
        <button @click="openDirectory()">
          <ListTree :size="17" />已下载目录
        </button>
        <button v-if="book.novelId" @click="desk.readOnline">
          <ExternalLink :size="17" />在线查看
        </button>
        <button v-if="book.novelId" @click="desk.continueDownload">
          <Download :size="17" />补充下载
        </button>
        <button
          :disabled="
            Boolean(exporting) || (!textChapterCount && !audioChapterCount)
          "
          @click="exportModalOpen = true"
        >
          <FileDown :size="17" />{{ exporting ? "正在导出" : "导出内容" }}
        </button>
      </section>
    </section>

    <section v-else class="comic-experience">
      <div class="comic-backdrop" aria-hidden="true">
        <img v-if="book.cover" :src="book.cover" alt="" />
      </div>
      <header class="detail-navigation comic-navigation">
        <button
          class="nav-circle"
          title="返回小说详情"
          @click="detailMode = 'novel'"
        >
          <ChevronLeft :size="22" />
        </button>
        <span class="local-label"><Images :size="14" />漫画改编</span>
      </header>
      <main class="comic-detail-sheet">
        <section class="comic-showcase">
          <div class="comic-cover-frame">
            <img
              v-if="book.cover"
              :src="book.cover"
              :alt="`${book.name} 漫画封面`"
            />
            <Images v-else :size="52" />
          </div>
          <div class="comic-showcase-copy">
            <span class="comic-edition-label"
              ><Images :size="14" />漫画改编</span
            >
            <h3>{{ book.name }}</h3>
            <p>{{ book.author || "未知作者" }}</p>
          </div>
        </section>
        <section class="comic-copy">
          <div class="comic-copy-heading">
            <span>COMIC ADAPTATION</span>
            <strong>作品设定</strong>
          </div>
          <p>{{ book.description || "暂无本地简介" }}</p>
        </section>
        <section class="comic-unavailable">
          <span class="comic-unavailable-icon"><Images :size="29" /></span>
          <div>
            <strong>{{ comicChapterCount ? `已下载 ${comicChapterCount} 个漫画章节` : '漫画章节暂未收录' }}</strong>
            <p>{{ comicChapterCount ? '选择章节开始阅读。' : '本地尚未下载漫画章节。' }}</p>
          </div>
          <button v-if="comicChapterCount" @click="readComicChapter(book.comicChapters[0].id)"><Play :size="17" />开始阅读</button>
          <button @click="detailMode = 'novel'">
            <BookOpen :size="17" />返回小说详情
          </button>
        </section>
      </main>
    </section>

    <nav
      v-if="detailMode === 'novel'"
      class="mobile-reading-bar"
      :class="{ 'without-download': !book.novelId }"
      aria-label="阅读操作"
    >
      <button class="directory-action" @click="openDirectory()">
        <ListTree :size="21" /><span>目录</span>
      </button>
      <button
        v-if="book.novelId"
        class="directory-action"
        @click="desk.continueDownload"
      >
        <Download :size="21" /><span>下载</span>
      </button>
      <button
        class="directory-action"
        :disabled="
          Boolean(exporting) || (!textChapterCount && !audioChapterCount)
        "
        title="导出本地内容"
        @click="exportModalOpen = true"
      >
        <FileDown :size="21" /><span>导出</span>
      </button>
      <button
        class="primary-action"
        :disabled="!textChapterCount && !audioChapterCount"
        @click="startReading"
      >
        <Play :size="17" fill="currentColor" />{{ primaryActionLabel }}
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
        aria-labelledby="directory-title"
      >
        <div class="drawer-handle" aria-hidden="true" />
        <header class="drawer-heading">
          <div>
            <small>已下载内容</small>
            <h3 id="directory-title">本地目录</h3>
          </div>
          <button title="关闭目录" @click="directoryOpen = false">
            <X :size="20" />
          </button>
        </header>
        <div class="drawer-tabs" aria-label="目录类型">
          <button
            :class="{ active: chapterMode === 'text' }"
            :disabled="!textChapterCount"
            @click="chapterMode = 'text'"
          >
            <BookOpen :size="15" />文字 {{ textChapterCount }}
          </button>
          <button
            :class="{ active: chapterMode === 'audio' }"
            :disabled="!audioChapterCount"
            @click="chapterMode = 'audio'"
          >
            <Headphones :size="15" />有声 {{ audioChapterCount }}
          </button>
          <button :class="{ active: chapterMode === 'comic' }" :disabled="!comicChapterCount" @click="chapterMode = 'comic'"><Images :size="15" />漫画 {{ comicChapterCount }}</button>
        </div>
        <div
          v-if="chapterMode === 'text' && textChapterCount"
          class="drawer-list"
        >
          <section
            v-for="volume in book.chapterVolumes"
            :key="volume.volume"
            class="chapter-volume"
          >
            <h4>{{ volume.volume }}</h4>
            <button
              v-for="chapter in volume.chapters"
              :key="chapter.id"
              @click="readChapter(chapter.id)"
            >
              <strong>{{ chapter.title }}</strong
              ><ChevronRight :size="16" />
            </button>
          </section>
        </div>
        <div
          v-else-if="chapterMode === 'audio' && audioChapterCount"
          class="drawer-list"
        >
          <button
            v-for="(track, index) in book.audioTracks"
            :key="track.href"
            class="audio-row"
            @click="playAudio(index)"
          >
            <span>{{ String(index + 1).padStart(3, "0") }}</span
            ><strong>{{ track.title }}</strong
            ><Headphones :size="16" />
          </button>
        </div>
        <div v-else-if="chapterMode === 'comic' && comicChapterCount" class="drawer-list">
          <button v-for="chapter in book.comicChapters" :key="chapter.id" @click="readComicChapter(chapter.id)"><strong>{{ chapter.title }}</strong><ChevronRight :size="16" /></button>
        </div>
        <div v-else class="drawer-empty">
          <ListTree :size="27" />暂无已下载内容
        </div>
      </section>
    </div>
  </section>
  <ExportModal
    :open="exportModalOpen"
    :text-chapter-count="textChapterCount"
    :audio-chapter-count="audioChapterCount"
    :exporting="exporting"
    @close="exportModalOpen = false"
    @export="
      (format) => {
        exportModalOpen = false;
        desk.exportBook(format);
      }
    "
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
.support-actions button,
.drawer-heading button,
.drawer-tabs button,
.comic-unavailable button,
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
.comic-copy h3,
.section-title h3,
.desktop-directory h3,
.drawer-heading h3,
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

  .desktop-nav-label {
    display: inline;
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

  .desktop-cover img,
  .comic-cover-frame img {
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

  .desktop-directory-list section + section {
    border-top: 8px solid #f8f3eb;
  }

  .desktop-directory-list h4 {
    margin: 0;
    padding: 12px 18px 7px;
    color: #9c8d7d;
    font-size: 11px;
  }

  .desktop-directory-list button {
    display: flex;
    width: 100%;
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

  .comic-page {
    padding-bottom: 0;
    background: #332b40;
  }

  .comic-experience {
    position: relative;
    min-height: 100dvh;
    overflow: hidden;
    padding-bottom: 52px;
  }

  .comic-backdrop {
    position: absolute;
    z-index: 0;
    inset: -36px;
    overflow: hidden;
    background: linear-gradient(135deg, #41354f, #765b80);
  }

  .comic-backdrop img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: blur(28px) saturate(0.72) brightness(0.64);
    opacity: 0.86;
    transform: scale(1.08);
  }

  .comic-backdrop::after {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      135deg,
      rgba(35, 27, 47, 0.7),
      rgba(76, 54, 90, 0.42)
    );
    content: "";
  }

  .comic-navigation {
    position: relative;
    z-index: 1;
    width: min(1180px, calc(100% - 64px));
    margin: 0 auto;
    padding: 18px 0 14px;
  }

  .comic-navigation .nav-circle,
  .comic-navigation .local-label {
    border-color: rgba(255, 255, 255, 0.26);
    color: #fff;
    background: rgba(255, 255, 255, 0.15);
    box-shadow: 0 8px 20px rgba(25, 18, 34, 0.14);
    backdrop-filter: blur(15px);
    -webkit-backdrop-filter: blur(15px);
  }

  .comic-detail-sheet {
    position: relative;
    z-index: 1;
    display: flex;
    width: min(1000px, calc(100% - 64px));
    min-height: 500px;
    flex-wrap: wrap;
    gap: 20px;
    align-items: stretch;
    margin: 12px auto 0;
    padding: 28px;
    border: 1px solid rgba(255, 255, 255, 0.36);
    border-radius: 22px;
    background: rgba(43, 34, 57, 0.42);
    box-shadow: 0 24px 52px rgba(19, 14, 28, 0.28);
    backdrop-filter: blur(24px) saturate(1.04);
    -webkit-backdrop-filter: blur(24px) saturate(1.04);
  }

  .comic-showcase {
    display: flex;
    width: 100%;
    min-width: 0;
    align-items: center;
    gap: 28px;
    padding-bottom: 6px;
  }

  .comic-cover-frame {
    display: flex;
    width: 190px;
    min-width: 0;
    flex: 0 0 auto;
    aspect-ratio: 3 / 4;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    border: 6px solid rgba(255, 255, 255, 0.74);
    border-radius: 12px;
    color: #f4ebff;
    background: rgba(92, 69, 117, 0.74);
    box-shadow: 0 14px 28px rgba(19, 14, 28, 0.32);
  }

  .comic-showcase-copy {
    min-width: 0;
    color: #fff;
  }

  .comic-edition-label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 9px;
    border: 1px solid rgba(255, 255, 255, 0.26);
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.14);
    font-size: 11px;
  }

  .comic-showcase-copy h3 {
    margin: 14px 0 0;
    font-family: KaTongFont, "Microsoft YaHei", sans-serif;
    font-size: clamp(30px, 4vw, 44px);
    line-height: 1.15;
  }

  .comic-showcase-copy p {
    margin: 9px 0 0;
    color: rgba(255, 255, 255, 0.76);
    font-size: 13px;
  }

  .comic-copy,
  .comic-unavailable {
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.1);
  }

  .comic-copy {
    min-width: 0;
    flex: 1 1 440px;
    padding: 20px;
  }

  .comic-copy-heading {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }

  .comic-copy-heading span {
    color: rgba(230, 215, 255, 0.68);
    font: 10px monospace;
  }

  .comic-copy-heading strong {
    color: #fff;
    font-family: KaTongFont, "Microsoft YaHei", sans-serif;
    font-size: 20px;
  }

  .comic-copy > p {
    margin: 14px 0 0;
    color: rgba(255, 255, 255, 0.78);
    font-size: 13px;
    line-height: 1.8;
    white-space: pre-line;
  }

  .comic-unavailable {
    display: flex;
    min-height: 146px;
    flex: 1 1 280px;
    align-items: center;
    gap: 12px;
    padding: 20px;
    color: #f4effb;
    text-align: left;
  }

  .comic-unavailable-icon {
    display: grid;
    width: 50px;
    height: 50px;
    flex: 0 0 auto;
    border-radius: 14px;
    color: #e3d2fa;
    background: rgba(222, 198, 245, 0.19);
    place-items: center;
  }

  .comic-unavailable > div {
    min-width: 0;
    flex: 1 1 auto;
  }

  .comic-unavailable p {
    margin: 6px 0 0;
    color: rgba(255, 255, 255, 0.68);
    font-size: 12px;
    line-height: 1.65;
  }

  .comic-unavailable button {
    display: inline-flex;
    min-height: 38px;
    align-items: center;
    gap: 5px;
    flex: 0 0 auto;
    padding: 0 12px;
    border: 1px solid rgba(255, 255, 255, 0.25);
    border-radius: 8px;
    color: #fff;
    background: rgba(192, 155, 221, 0.36);
    font-size: 12px;
  }
}

/* Mobile layout: tall cover hero, overlapping detail sheet, and action dock. */
@media (max-width: 900px) {
  .detail-page {
    min-height: 100dvh;
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
    padding: 25px 20px 30px;
    border-radius: 28px 28px 0 0;
    box-shadow: 0 -8px 30px rgba(55, 49, 41, 0.1);
    padding-bottom: 50px;
    background: rgba(255, 255, 255, 0.55);
    border: 1px solid rgba(255, 255, 255, 0.8);
    box-shadow: 0 15px 38px rgba(137, 76, 55, 0.12);
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
    transition:
      transform 0.2s ease,
      box-shadow 0.2s ease;
  }

  .format-switch:hover:not(:disabled) {
    box-shadow: 0 13px 25px rgba(85, 73, 53, 0.16);
    transform: translateY(-1px);
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
    letter-spacing: 0;
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
    box-shadow: inset 0 1px rgba(255, 255, 255, 0.7);
  }

  .audio-switch .format-copy small,
  .audio-switch > svg {
    color: #8b6826;
  }

  .comic-switch {
    color: #433860;
    background: rgba(184, 162, 221, 0.42);
  }

  .comic-switch .format-icon {
    color: #604a91;
    background: rgba(246, 240, 255, 0.64);
    box-shadow: inset 0 1px rgba(255, 255, 255, 0.74);
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
    min-height: 54px;
    align-items: center;
    gap: 9px;
    margin-top: 15px;
    padding: 0 14px;
    border-radius: 16px;
    color: #756c61;
    /* background: #f1eee3; */
    background: rgba(250, 250, 250, 0.5);
    border: 1px solid rgba(255, 255, 255, 0.3);
    backdrop-filter: blur(13px);
    box-shadow: 0 4px 12px rgba(85, 73, 53, 0.08);
    text-align: left;
  }

  .latest-badge {
    padding: 6px 8px;
    border-radius: 14px;
    color: #fff;
    background: #e86e4c;
    font-size: 10px;
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

  .support-actions {
    display: none;
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

  .mobile-reading-bar.without-download {
    gap: 6px;
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
    font-size: 10px;
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
    border-radius: 14px;
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

  .comic-page {
    padding-bottom: 0;
    background: #30263b;
  }

  .comic-experience {
    position: relative;
    min-height: 100dvh;
    overflow: hidden;
    padding-bottom: calc(20px + env(safe-area-inset-bottom));
  }

  .comic-backdrop {
    position: absolute;
    z-index: 0;
    inset: -28px;
    overflow: hidden;
    background: linear-gradient(150deg, #49355b, #201d31);
  }

  .comic-backdrop img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: blur(26px) saturate(0.78) brightness(0.62);
    opacity: 0.9;
    transform: scale(1.1);
  }

  .comic-backdrop::after {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      180deg,
      rgba(34, 25, 46, 0.54),
      rgba(53, 39, 72, 0.78)
    );
    content: "";
  }

  .comic-navigation {
    position: relative;
    z-index: 1;
    padding: 12px 16px 4px;
  }

  .comic-navigation .nav-circle,
  .comic-navigation .local-label {
    border-color: rgba(255, 255, 255, 0.25);
    color: #fff;
    background: rgba(255, 255, 255, 0.16);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
  }

  .comic-detail-sheet {
    position: relative;
    z-index: 1;
    display: flex;
    min-height: calc(100dvh - 80px);
    flex-direction: column;
    gap: 15px;
    margin: 14px 12px 0;
    padding: 20px 16px 28px;
    border: 1px solid rgba(255, 255, 255, 0.3);
    border-radius: 25px;
    background: rgba(44, 35, 61, 0.52);
    box-shadow: 0 18px 38px rgba(20, 15, 31, 0.26);
    backdrop-filter: blur(22px) saturate(1.06);
    -webkit-backdrop-filter: blur(22px) saturate(1.06);
  }

  .comic-showcase {
    display: flex;
    min-width: 0;
    align-items: center;
    flex-direction: column;
    gap: 16px;
    padding: 4px 4px 10px;
    color: #fff;
    text-align: center;
  }

  .comic-cover-frame {
    display: flex;
    width: min(210px, 58vw);
    min-width: 0;
    flex: 0 0 auto;
    aspect-ratio: 3 / 4;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    border: 7px solid rgba(255, 255, 255, 0.82);
    border-radius: 15px;
    color: #f7efff;
    background: rgba(88, 69, 117, 0.76);
    box-shadow: 0 17px 33px rgba(15, 10, 26, 0.34);
  }

  .comic-showcase-copy {
    min-width: 0;
    width: 100%;
  }

  .comic-edition-label {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 6px 9px;
    border: 1px solid rgba(255, 255, 255, 0.28);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.15);
    font-size: 11px;
  }

  .comic-showcase-copy h3 {
    display: -webkit-box;
    overflow: hidden;
    margin: 12px 0 0;
    font-family: KaTongFont, "Microsoft YaHei", sans-serif;
    font-size: clamp(24px, 7vw, 32px);
    line-height: 1.18;
    text-overflow: ellipsis;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
  }

  .comic-showcase-copy p {
    margin: 7px 0 0;
    color: rgba(255, 255, 255, 0.72);
    font-size: 12px;
  }

  .comic-copy,
  .comic-unavailable {
    border: 1px solid rgba(255, 255, 255, 0.26);
    border-radius: 17px;
    background: rgba(255, 255, 255, 0.12);
    box-shadow: 0 10px 22px rgba(20, 14, 30, 0.14);
    backdrop-filter: blur(15px);
    -webkit-backdrop-filter: blur(15px);
  }

  .comic-copy {
    padding: 18px;
  }

  .comic-copy-heading {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
  }

  .comic-copy-heading span {
    color: rgba(225, 207, 249, 0.68);
    font: 9px monospace;
    letter-spacing: 0.08em;
  }

  .comic-copy-heading strong {
    color: #fff;
    font-family: KaTongFont, "Microsoft YaHei", sans-serif;
    font-size: 18px;
  }

  .comic-copy > p {
    display: -webkit-box;
    overflow: hidden;
    margin: 12px 0 0;
    color: rgba(255, 255, 255, 0.76);
    font-size: 12px;
    line-height: 1.8;
    white-space: pre-line;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 5;
  }

  .comic-unavailable {
    display: flex;
    min-height: 122px;
    align-items: flex-start;
    gap: 12px;
    padding: 17px;
    color: #f5effc;
    text-align: left;
  }

  .comic-unavailable-icon {
    display: grid;
    width: 46px;
    height: 46px;
    flex: 0 0 auto;
    border-radius: 14px;
    color: #e3d2fa;
    background: rgba(222, 198, 245, 0.2);
    place-items: center;
  }

  .comic-unavailable > div {
    min-width: 0;
    flex: 1 1 auto;
  }

  .comic-unavailable strong,
  .comic-unavailable p {
    display: block;
  }

  .comic-unavailable strong {
    color: #fff;
    font-size: 14px;
  }

  .comic-unavailable p {
    display: -webkit-box;
    overflow: hidden;
    margin: 5px 0 0;
    color: rgba(255, 255, 255, 0.66);
    font-size: 11px;
    line-height: 1.55;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
  }

  .comic-unavailable button {
    display: inline-flex;
    min-height: 40px;
    align-items: center;
    gap: 5px;
    flex: 0 0 auto;
    align-self: flex-end;
    padding: 0 11px;
    border: 1px solid rgba(255, 255, 255, 0.26);
    border-radius: 10px;
    color: #fff;
    background: rgba(192, 155, 221, 0.36);
    font-size: 11px;
  }
}

/* Modal and chapter drawer shared by both form factors. */
.description-modal-backdrop,
.directory-backdrop {
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

.description-modal header,
.drawer-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.description-modal header {
  padding-bottom: 15px;
  border-bottom: 1px solid #eee5d9;
}

.description-modal header small,
.drawer-heading small {
  color: #b59885;
  font: 10px monospace;
  letter-spacing: 0.08em;
}

.description-modal h3,
.drawer-heading h3 {
  margin: 4px 0 0;
  color: #4c4037;
  font-size: 22px;
}

.description-modal header button,
.drawer-heading button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
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
  z-index: 50;
  align-items: flex-end;
}

.directory-drawer {
  display: flex;
  width: min(100%, 720px);
  max-height: min(78dvh, 680px);
  flex-direction: column;
  overflow: hidden;
  padding: 8px 0 0;
  border-radius: 19px 19px 0 0;
  background: #fffdf8;
  box-shadow: 0 -18px 40px rgba(46, 40, 33, 0.2);
}

.drawer-handle {
  width: 37px;
  height: 4px;
  margin: 0 auto 6px;
  border-radius: 3px;
  background: #d6cfc0;
}

.drawer-heading {
  padding: 8px 20px 13px;
  border-bottom: 1px solid #eee8dc;
}

.drawer-heading h3 {
  color: #443e36;
  font-size: 20px;
}

.drawer-tabs {
  display: flex;
  gap: 7px;
  padding: 13px 20px;
}

.drawer-tabs button {
  display: inline-flex;
  min-height: 32px;
  align-items: center;
  gap: 5px;
  padding: 0 11px;
  border: 1px solid #e4ddce;
  border-radius: 6px;
  color: #857d70;
  background: #fffdf8;
  font-size: 12px;
}

.drawer-tabs button.active {
  border-color: #e67c55;
  color: #fff;
  background: #e67c55;
}

.drawer-tabs button:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.drawer-list {
  overflow-y: auto;
  padding-bottom: env(safe-area-inset-bottom);
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

.chapter-volume button,
.audio-row {
  display: flex;
  width: 100%;
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

.chapter-volume button:hover,
.audio-row:hover {
  color: #bd6548;
  background: #fff9f1;
}

.chapter-volume strong,
.audio-row strong {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  font-size: 13px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.chapter-volume button svg,
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

/* Mobile drawer: full-width bottom sheet, independent from the page dock. */
@media (max-width: 900px) {
  .directory-backdrop {
    padding: 0;
    background: rgba(43, 36, 31, 0.46);
  }

  .directory-drawer {
    width: 100%;
    max-height: min(82dvh, 720px);
    border-radius: 24px 24px 0 0;
    box-shadow: 0 -20px 48px rgba(46, 40, 33, 0.26);
  }

  .drawer-handle {
    width: 42px;
    height: 5px;
    margin: 8px auto 7px;
    background: #d2c9b9;
  }

  .drawer-heading {
    padding: 9px 20px 16px;
  }

  .drawer-heading h3 {
    font-size: 22px;
  }

  .drawer-heading button {
    width: 38px;
    height: 38px;
    background: #f4eee4;
  }

  .drawer-tabs {
    gap: 8px;
    padding: 12px 20px;
    border-bottom: 1px solid #eee8dc;
    background: #fbf8f1;
  }

  .drawer-tabs button {
    min-height: 38px;
    flex: 1 1 0;
    justify-content: center;
    border-radius: 10px;
  }

  .drawer-list {
    background: #fffdf8;
  }
}
</style>
