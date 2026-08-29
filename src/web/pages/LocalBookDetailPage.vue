<script setup lang="ts">
import { computed, ref } from "vue";
import {
  BookOpen,
  ChevronLeft,
  Download,
  FileDown,
  Headphones,
  ListTree,
} from "lucide-vue-next";
import type { LocalBookDetail } from "../types";

const { book } = defineProps<{ book: LocalBookDetail; exporting: boolean }>();
const textChapterCount = computed(() =>
  book.chapterVolumes.reduce(
    (count, volume) => count + volume.chapters.length,
    0,
  ),
);
const audioChapterCount = computed(() => book.audioTracks.length);
const chapterMode = ref<"text" | "audio">(
  textChapterCount.value ? "text" : "audio",
);
const chapterCount = computed(() =>
  chapterMode.value === "text"
    ? textChapterCount.value
    : audioChapterCount.value,
);
const emit = defineEmits<{
  back: [];
  read: [chapterId: number];
  playAudio: [trackIndex: number];
  online: [];
  continueDownload: [];
  export: [];
}>();
</script>

<template>
  <section class="detail-page">
    <button class="back-button" @click="emit('back')">
      <ChevronLeft :size="17" />本地书库
    </button>
    <section class="book-summary glass">
      <div class="detail-cover">
        <img
          v-if="book.cover"
          :src="book.cover"
          :alt="`${book.name} 封面`"
        /><BookOpen v-else :size="42" />
      </div>
      <div class="detail-copy">
        <p class="eyebrow">LOCAL BOOK</p>
        <h2>{{ book.name }}</h2>
        <p class="author">{{ book.author }}</p>
        <p class="description">{{ book.description }}</p>
        <div class="detail-actions">
          <button
            v-if="book.novelId"
            class="action-button"
            title="在线阅读"
            @click="emit('online')"
          >
            <BookOpen :size="17" />在线阅读
          </button>
          <button
            v-if="book.novelId"
            class="action-button"
            title="继续下载"
            @click="emit('continueDownload')"
          >
            <Download :size="17" />下载章节
          </button>
          <button
            class="action-button"
            :disabled="!chapterCount || exporting"
            title="导出 EPUB"
            @click="emit('export')"
          >
            <FileDown :size="17" />{{ exporting ? "正在导出" : "导出 EPUB" }}
          </button>
          <button
            v-if="book.audioTracks.length"
            class="action-button"
            title="播放本地有声内容"
            @click="emit('playAudio', 0)"
          >
            <Headphones :size="17" />播放有声
          </button>
        </div>
      </div>
    </section>
    <section class="chapter-section">
      <div class="chapter-heading">
        <span><ListTree :size="19" />已下载章节目录</span>
        <div class="chapter-heading-actions">
          <div class="chapter-mode" aria-label="章节类型">
            <button
              :class="{ active: chapterMode === 'text' }"
              :disabled="!textChapterCount"
              @click="chapterMode = 'text'"
            >
              <BookOpen :size="15" />文字章节
            </button>
            <button
              :class="{ active: chapterMode === 'audio' }"
              :disabled="!audioChapterCount"
              @click="chapterMode = 'audio'"
            >
              <Headphones :size="15" />有声章节
            </button>
          </div>
          <small>{{ chapterCount }} 章</small>
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
          <h3
            style="
              font-size: 16px;
              display: flex;
              align-items: center;
              gap: 6px;
            "
          >
            <BookOpen :size="17" />{{ volume.volume }}
          </h3>
          <button
            v-for="chapter in volume.chapters"
            :key="chapter.id"
            class="chapter-row"
            @click="emit('read', chapter.id)"
          >
            <strong>{{ chapter.title }}</strong>
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
        <p>
          尚未保存{{
            chapterMode === "text" ? "可阅读的文字" : "可播放的有声"
          }}章节
        </p>
      </div>
    </section>
  </section>
</template>

<style scoped>
.detail-page {
  padding-bottom: 36px;
}
.back-button {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  margin-bottom: 13px;
  padding: 0;
  border: 0;
  color: #94675c;
  background: none;
  font-size: 12px;
  font-weight: 600;
}
.book-summary {
  display: grid;
  grid-template-columns: 154px minmax(0, 1fr);
  gap: 24px;
  padding: 20px;
  border-radius: 14px;
}
.detail-cover {
  display: grid;
  aspect-ratio: 3 / 4;
  overflow: hidden;
  border-radius: 8px;
  color: var(--theme-color-dark);
  background: #ffe7d8;
  place-items: center;
}
.detail-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.detail-copy {
  min-width: 0;
}
.eyebrow {
  margin: 0 0 5px;
  color: #b08072;
  font: 10px monospace;
  letter-spacing: 0.1em;
}
h2 {
  margin: 0;
  color: #69453d;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 25px;
}
.author {
  margin: 6px 0 12px;
  color: #987972;
  font-size: 13px;
}
.description {
  display: -webkit-box;
  overflow: hidden;
  margin: 0;
  color: #785e57;
  font-size: 13px;
  line-height: 1.65;
  white-space: pre-line;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
}
.detail-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 15px;
}
.action-button {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-height: 34px;
  padding: 0 10px;
  border: 1px solid #ead1c5;
  border-radius: 7px;
  color: #895c51;
  background: #fffaf7;
  font-size: 12px;
  font-weight: 600;
  text-decoration: none;
}
.action-button:hover:not(:disabled) {
  border-color: var(--theme-color);
  color: #fff;
  background: var(--theme-color);
}
.action-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.chapter-section {
  margin-top: 24px;
}
.chapter-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
  color: #69453d;
}
.chapter-heading-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}
.chapter-heading span {
  display: flex;
  align-items: center;
  gap: 6px;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 18px;
}
.chapter-mode {
  display: inline-flex;
  overflow: hidden;
  border: 1px solid #ead1c5;
  border-radius: 6px;
  background: #fffaf7;
}
.chapter-mode button {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  min-height: 28px;
  padding: 0 8px;
  border: 0;
  color: #987972;
  background: transparent;
  font-size: 11px;
  font-weight: 600;
}
.chapter-mode button + button {
  border-left: 1px solid #ead1c5;
}
.chapter-mode button.active {
  color: #fff;
  background: var(--theme-color);
}
.chapter-mode button:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
.chapter-heading small {
  color: #ad877a;
  font-size: 12px;
}
.chapter-list {
  overflow: hidden;
  border: 1px solid rgba(235, 205, 192, 0.9);
  border-radius: 9px;
  background: rgba(255, 255, 255, 0.45);
}
.chapter-row {
  width: 100%;
  min-height: 43px;
  padding: 0 14px;
  border: 0;
  border-bottom: 1px solid rgba(187, 132, 111, 0.13);
  color: #65433c;
  background: none;
  text-align: left;
}
.chapter-row:hover {
  background: #fff5ef;
}
.chapter-row strong {
  display: block;
  width: 100%;
  overflow: hidden;
  font-size: 13px;
  font-weight: 500;
  line-height: 43px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.audio-chapter-row {
  display: grid;
  grid-template-columns: 40px minmax(0, 1fr) 20px;
  align-items: center;
  gap: 8px;
}
.audio-chapter-row span {
  color: #a47b6d;
  font: 11px monospace;
}
.audio-chapter-row strong {
  line-height: 1.45;
}
.audio-chapter-row svg {
  color: #a17367;
}
.chapter-volume + .chapter-volume {
  border-top: 1px solid rgba(187, 132, 111, 0.22);
}
.chapter-volume h3 {
  margin: 0;
  padding: 11px 14px;
  color: #895c51;
  background: #fff5ef;
  font-size: 12px;
  font-weight: 700;
}
.chapter-volume h3 + .chapter-row {
  border-top: 1px solid rgba(187, 132, 111, 0.13);
}
.chapter-volume .chapter-row:last-child {
  border-bottom: 0;
}
.empty-state {
  min-height: 120px;
}
.empty-state p {
  margin: 0;
}
@media (max-width: 650px) {
  .book-summary {
    grid-template-columns: 96px 1fr;
    gap: 14px;
    padding: 14px;
  }
  .chapter-row {
    padding: 8px 12px;
  }
  .chapter-row strong {
    line-height: 1.45;
  }
  .chapter-heading {
    align-items: flex-start;
    gap: 8px;
  }
  .chapter-heading-actions {
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
  }
  .chapter-mode button {
    padding: 0 6px;
  }
  .detail-actions {
    margin-top: 11px;
  }
}
</style>
