<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import {
  BookOpen,
  ChevronLeft,
  Download,
  ExternalLink,
  FileDown,
  Headphones,
  LibraryBig,
  ListTree,
  Play,
} from "lucide-vue-next";
import ExportModal from "../components/ExportModal.vue";
import type { LocalBookDetail } from "../types";

const { book } = defineProps<{
  book: LocalBookDetail;
  exporting?: "epub" | "markdown" | "txt" | "audio";
}>();

const textChapterCount = computed(() =>
  book.chapterVolumes.reduce(
    (count, volume) => count + volume.chapters.length,
    0,
  ),
);
const audioChapterCount = computed(() => book.audioTracks.length);
const firstTextChapterId = computed(
  () => book.chapterVolumes[0]?.chapters[0]?.id,
);
const chapterMode = ref<"text" | "audio">(
  textChapterCount.value ? "text" : "audio",
);
const exportModalOpen = ref(false);
const descriptionExpanded = ref(false);
const chapterSection = ref<HTMLElement>();
const chapterCount = computed(() =>
  chapterMode.value === "text"
    ? textChapterCount.value
    : audioChapterCount.value,
);
const primaryActionLabel = computed(() =>
  textChapterCount.value ? "开始阅读" : "播放有声",
);
const emit = defineEmits<{
  back: [];
  read: [chapterId: number];
  playAudio: [trackIndex: number];
  online: [];
  continueDownload: [];
  export: [format: "epub" | "markdown" | "txt" | "audio"];
}>();

function startReading() {
  if (firstTextChapterId.value) {
    emit("read", firstTextChapterId.value);
    return;
  }
  if (audioChapterCount.value) emit("playAudio", 0);
}

async function openDirectory() {
  chapterMode.value = textChapterCount.value ? "text" : "audio";
  await nextTick();
  chapterSection.value?.scrollIntoView({ behavior: "smooth", block: "start" });
}
</script>

<template>
  <section class="detail-page">
    <header class="detail-navigation">
      <button class="back-button" @click="emit('back')">
        <ChevronLeft :size="18" />
        <span>返回本地书库</span>
      </button>
      <span class="local-label"><LibraryBig :size="14" />本地作品</span>
    </header>

    <section class="book-hero" :class="{ 'without-cover': !book.cover }">
      <div
        v-if="book.cover"
        class="hero-art"
        :style="{ backgroundImage: `url('${book.cover}')` }"
        aria-hidden="true"
      ></div>
      <div class="hero-shade"></div>
      <div class="hero-content">
        <div class="detail-cover">
          <img
            v-if="book.cover"
            :src="book.cover"
            :alt="`${book.name} 封面`"
          />
          <BookOpen v-else :size="46" />
        </div>

        <div class="hero-copy">
          <p class="eyebrow">LOCAL LIBRARY</p>
          <h2>{{ book.name }}</h2>
          <p class="author">{{ book.author || "未知作者" }}</p>
          <div class="media-badges" aria-label="本地资源类型">
            <span v-if="textChapterCount"><BookOpen :size="14" />文字</span>
            <span v-if="audioChapterCount"><Headphones :size="14" />有声</span>
          </div>
          <div class="hero-actions">
            <button
              class="primary-action"
              :disabled="!textChapterCount && !audioChapterCount"
              @click="startReading"
            >
              <Play :size="17" fill="currentColor" />{{ primaryActionLabel }}
            </button>
            <button
              v-if="book.audioTracks.length && textChapterCount"
              class="secondary-action"
              title="播放本地有声内容"
              @click="emit('playAudio', 0)"
            >
              <Headphones :size="17" /><span>有声播放</span>
            </button>
            <button
              v-if="book.novelId"
              class="secondary-action"
              title="打开作品在线页面"
              @click="emit('online')"
            >
              <ExternalLink :size="17" /><span>在线查看</span>
            </button>
            <button
              v-if="book.novelId"
              class="secondary-action"
              title="补充下载章节"
              @click="emit('continueDownload')"
            >
              <Download :size="17" /><span>补充章节</span>
            </button>
            <button
              class="secondary-action"
              :disabled="
                Boolean(exporting) || (!textChapterCount && !audioChapterCount)
              "
              title="导出本地内容"
              @click="exportModalOpen = true"
            >
              <FileDown :size="17" /><span>{{ exporting ? "正在导出" : "导出" }}</span>
            </button>
          </div>
        </div>
      </div>

      <dl class="book-stats" aria-label="本地书籍统计">
        <div>
          <dt>{{ textChapterCount }}</dt>
          <dd>文字章节</dd>
        </div>
        <div>
          <dt>{{ audioChapterCount }}</dt>
          <dd>有声音轨</dd>
        </div>
        <div>
          <dt>{{ textChapterCount + audioChapterCount }}</dt>
          <dd>已保存内容</dd>
        </div>
      </dl>
    </section>

    <section class="book-intro" aria-labelledby="book-intro-title">
      <div class="section-title">
        <span class="section-mark"></span>
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

    <section
      ref="chapterSection"
      class="chapter-section"
      aria-labelledby="chapter-list-title"
    >
      <div class="chapter-heading">
        <div>
          <div class="section-title">
            <span class="section-mark"></span>
            <h3 id="chapter-list-title">本地目录</h3>
          </div>
          <small>仅显示已保存到本机的内容</small>
        </div>
        <div class="chapter-heading-actions">
          <div class="chapter-mode" aria-label="章节类型">
            <button
              :class="{ active: chapterMode === 'text' }"
              :disabled="!textChapterCount"
              @click="chapterMode = 'text'"
            >
              <BookOpen :size="15" />文字
            </button>
            <button
              :class="{ active: chapterMode === 'audio' }"
              :disabled="!audioChapterCount"
              @click="chapterMode = 'audio'"
            >
              <Headphones :size="15" />有声
            </button>
          </div>
          <strong>{{ chapterCount }} 章</strong>
        </div>
      </div>

      <div
        v-if="chapterMode === 'text' && textChapterCount"
        class="chapter-list"
      >
        <section
          v-for="volume in book.chapterVolumes"
          :key="volume.volume"
          class="chapter-volume"
        >
          <h4><BookOpen :size="16" />{{ volume.volume }}</h4>
          <button
            v-for="chapter in volume.chapters"
            :key="chapter.id"
            class="chapter-row"
            @click="emit('read', chapter.id)"
          >
            <strong>{{ chapter.title }}</strong>
            <ChevronLeft :size="16" />
          </button>
        </section>
      </div>
      <div
        v-else-if="chapterMode === 'audio' && audioChapterCount"
        class="chapter-list"
      >
        <button
          v-for="(track, index) in book.audioTracks"
          :key="track.href"
          class="chapter-row audio-chapter-row"
          @click="emit('playAudio', index)"
        >
          <span>{{ String(index + 1).padStart(3, "0") }}</span>
          <strong>{{ track.title }}</strong>
          <Headphones :size="16" />
        </button>
      </div>
      <div v-else class="empty-state">
        <ListTree :size="28" />
        <p>尚未保存{{ chapterMode === "text" ? "文字" : "有声" }}章节</p>
      </div>
    </section>

    <div class="mobile-reading-bar">
      <button
        class="primary-action"
        :disabled="!textChapterCount && !audioChapterCount"
        @click="startReading"
      >
        <Play :size="17" fill="currentColor" />{{ primaryActionLabel }}
      </button>
      <button
        class="mobile-directory-action"
        @click="openDirectory"
      >
        <ListTree :size="20" />
        <span>目录</span>
      </button>
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
        emit('export', format);
      }
    "
  />
</template>

<style scoped>
.detail-page { padding: 0 0 42px; }
.detail-navigation { display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px; }
.back-button, .local-label, .media-badges span, .secondary-action, .primary-action, .chapter-mode button, .chapter-row, .mobile-directory-action { display: inline-flex; align-items: center; }
.back-button { gap: 3px; min-height: 32px; padding: 0 2px; border: 0; color: #80594f; background: transparent; font-size: 12px; font-weight: 700; }
.back-button:hover { color: var(--theme-color-dark); }
.local-label { gap: 5px; color: #a57c70; font-size: 11px; }
.book-hero { position: relative; overflow: hidden; min-height: 308px; border-radius: 8px; color: #fffaf5; background: #8b5d50; box-shadow: 0 18px 32px rgba(104, 61, 48, 0.18); }
.hero-art, .hero-shade { position: absolute; inset: -28px; }
.hero-art { background-position: center; background-size: cover; filter: blur(14px) saturate(0.76); opacity: 0.76; transform: scale(1.08); }
.hero-shade { inset: 0; background: linear-gradient(90deg, rgba(50, 30, 27, 0.82), rgba(57, 33, 29, 0.55) 57%, rgba(77, 46, 37, 0.32)); }
.without-cover .hero-shade { background: linear-gradient(135deg, #925f51, #ba765c); }
.hero-content { position: relative; z-index: 1; display: flex; align-items: center; gap: 25px; min-height: 236px; padding: 28px 31px 24px; }
.detail-cover { flex: 0 0 138px; display: grid; aspect-ratio: 3 / 4; overflow: hidden; border: 1px solid rgba(255, 255, 255, 0.42); border-radius: 6px; color: #fbe2d1; background: #644037; box-shadow: 0 12px 25px rgba(35, 21, 18, 0.35); place-items: center; }
.detail-cover img { width: 100%; height: 100%; object-fit: cover; }
.hero-copy { min-width: 0; max-width: 640px; }
.eyebrow { margin: 0 0 7px; color: rgba(255, 244, 234, 0.7); font: 10px monospace; letter-spacing: 0.1em; }
.hero-copy h2 { margin: 0; color: #fffaf6; font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 29px; line-height: 1.22; }
.author { margin: 7px 0 11px; color: rgba(255, 247, 239, 0.78); font-size: 13px; }
.media-badges { display: flex; flex-wrap: wrap; gap: 6px; }
.media-badges span { gap: 4px; padding: 4px 7px; border: 1px solid rgba(255, 255, 255, 0.2); border-radius: 4px; color: rgba(255, 250, 245, 0.9); background: rgba(44, 26, 22, 0.18); font-size: 11px; }
.hero-actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 18px; }
.primary-action { justify-content: center; gap: 6px; min-height: 38px; padding: 0 16px; border: 0; border-radius: 6px; color: #fff; background: var(--theme-color); box-shadow: 0 6px 14px rgba(54, 27, 20, 0.23); font-size: 13px; font-weight: 700; }
.primary-action:hover:not(:disabled) { background: #c96f48; transform: translateY(-1px); }
.primary-action:disabled, .secondary-action:disabled { cursor: not-allowed; opacity: 0.48; }
.secondary-action { justify-content: center; gap: 5px; min-height: 38px; padding: 0 11px; border: 1px solid rgba(255, 255, 255, 0.28); border-radius: 6px; color: #fffaf5; background: rgba(66, 37, 31, 0.24); font-size: 12px; font-weight: 600; }
.secondary-action:hover:not(:disabled) { background: rgba(255, 255, 255, 0.18); }
.book-stats { position: relative; z-index: 1; display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); margin: 0; border-top: 1px solid rgba(255, 255, 255, 0.19); background: rgba(48, 28, 24, 0.3); }
.book-stats div { min-width: 0; padding: 13px 30px 15px; }
.book-stats div + div { border-left: 1px solid rgba(255, 255, 255, 0.16); }
.book-stats dt { color: #fffaf5; font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 23px; line-height: 1.05; }
.book-stats dd { margin: 4px 0 0; color: rgba(255, 246, 239, 0.7); font-size: 11px; }
.book-intro { padding: 23px 1px 21px; border-bottom: 1px solid rgba(174, 122, 101, 0.2); }
.section-title { display: flex; align-items: center; gap: 8px; }
.section-mark { width: 3px; height: 19px; border-radius: 2px; background: var(--theme-color); }
.section-title h3 { margin: 0; color: #69453d; font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 19px; }
.book-intro p { display: -webkit-box; overflow: hidden; margin: 13px 0 0; color: #795e55; font-size: 13px; line-height: 1.8; white-space: pre-line; -webkit-box-orient: vertical; -webkit-line-clamp: 3; }
.book-intro p.expanded { display: block; }
.expand-description { padding: 5px 0 0; border: 0; color: var(--theme-color-dark); background: transparent; font-size: 12px; font-weight: 700; }
.chapter-section { margin-top: 22px; }
.chapter-heading { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; margin-bottom: 11px; }
.chapter-heading small { display: block; margin: 5px 0 0 11px; color: #ab867a; font-size: 11px; }
.chapter-heading-actions { display: flex; align-items: center; gap: 11px; }
.chapter-heading-actions > strong { color: #966a5b; font-size: 12px; white-space: nowrap; }
.chapter-mode { display: inline-flex; overflow: hidden; border: 1px solid #e8cec1; border-radius: 6px; background: #fffaf7; }
.chapter-mode button { gap: 4px; min-height: 30px; padding: 0 9px; border: 0; color: #987972; background: transparent; font-size: 11px; font-weight: 700; }
.chapter-mode button + button { border-left: 1px solid #e8cec1; }
.chapter-mode button.active { color: #fff; background: var(--theme-color); }
.chapter-mode button:disabled { cursor: not-allowed; opacity: 0.45; }
.chapter-list { overflow: hidden; border: 1px solid rgba(223, 187, 172, 0.8); border-radius: 8px; background: rgba(255, 255, 255, 0.5); }
.chapter-volume + .chapter-volume { border-top: 1px solid rgba(187, 132, 111, 0.2); }
.chapter-volume h4 { display: flex; align-items: center; gap: 6px; margin: 0; padding: 10px 14px; color: #895c51; background: #fff5ef; font-size: 12px; }
.chapter-row { width: 100%; min-height: 44px; gap: 8px; padding: 0 14px; border: 0; border-top: 1px solid rgba(187, 132, 111, 0.12); color: #65433c; background: transparent; text-align: left; }
.chapter-row:hover { color: var(--theme-color-dark); background: #fff7f2; }
.chapter-row strong { flex: 1; overflow: hidden; font-size: 13px; font-weight: 500; line-height: 1.4; text-overflow: ellipsis; white-space: nowrap; }
.chapter-row > svg { color: #b58575; transform: rotate(180deg); }
.audio-chapter-row { display: grid; grid-template-columns: 40px minmax(0, 1fr) 20px; }
.audio-chapter-row span { color: #a47b6d; font: 11px monospace; }
.audio-chapter-row > svg { transform: none; }
.empty-state { display: grid; min-height: 150px; gap: 8px; border: 1px dashed #e3bca9; border-radius: 8px; color: #ad877a; background: rgba(255, 250, 247, 0.26); font-size: 13px; justify-items: center; place-content: center; }
.empty-state p { margin: 0; }
.mobile-reading-bar { display: none; }
@media (max-width: 760px) {
  .detail-page { padding-bottom: calc(112px + env(safe-area-inset-bottom)); }
  .detail-navigation { margin: 0 1px 10px; }
  .book-hero { min-height: 0; margin-inline: -12px; border-radius: 0; }
  .hero-content { align-items: flex-start; gap: 14px; min-height: 0; padding: 21px 16px 19px; }
  .detail-cover { flex-basis: 100px; }
  .hero-copy h2 { font-size: 22px; }
  .author { margin: 5px 0 8px; }
  .hero-actions { display: none; }
  .book-stats div { padding: 11px 16px 13px; }
  .book-stats dt { font-size: 19px; }
  .book-intro { padding-top: 20px; }
  .chapter-heading { align-items: flex-start; }
  .chapter-heading-actions { flex-direction: column-reverse; align-items: flex-end; gap: 5px; }
  .chapter-heading small { max-width: 160px; margin-left: 11px; }
  .mobile-reading-bar { position: fixed; right: 0; bottom: 0; left: 0; z-index: 9; display: grid; grid-template-columns: minmax(0, 1fr) 57px; gap: 9px; margin: 0; padding: 10px 12px calc(10px + env(safe-area-inset-bottom)); border-top: 1px solid rgba(224, 184, 166, 0.8); background: rgba(255, 248, 244, 0.96); box-shadow: 0 -8px 24px rgba(137, 76, 55, 0.1); backdrop-filter: blur(14px); }
  .mobile-reading-bar .primary-action { min-height: 44px; }
  .mobile-directory-action { flex-direction: column; justify-content: center; gap: 1px; min-height: 44px; padding: 0; border: 0; color: #80594f; background: transparent; font-size: 10px; }
}
@media (max-width: 420px) {
  .back-button span { display: none; }
  .book-stats div { padding-inline: 12px; }
  .book-stats dd { font-size: 10px; }
  .detail-cover { flex-basis: 88px; }
  .hero-copy h2 { font-size: 20px; }
}
</style>
