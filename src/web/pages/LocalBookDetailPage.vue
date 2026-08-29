<script setup lang="ts">
import { BookOpen, ChevronLeft, Download, FileDown, Headphones, ListTree } from "lucide-vue-next";
import type { LocalBookDetail } from "../types";

defineProps<{ book: LocalBookDetail; exporting: boolean }>();
const emit = defineEmits<{
  back: [];
  read: [chapterId: number];
  online: [];
  continueDownload: [];
  export: [];
}>();
</script>

<template>
  <section class="detail-page">
    <button class="back-button" @click="emit('back')"><ChevronLeft :size="17" />本地书库</button>
    <section class="book-summary glass">
      <div class="detail-cover"><img v-if="book.cover" :src="book.cover" :alt="`${book.name} 封面`" /><BookOpen v-else :size="42" /></div>
      <div class="detail-copy">
        <p class="eyebrow">LOCAL BOOK</p>
        <h2>{{ book.name }}</h2>
        <p class="author">{{ book.author }}</p>
        <p class="description">{{ book.description }}</p>
        <div class="detail-actions">
          <button v-if="book.novelId" class="action-button" title="在线阅读" @click="emit('online')"><BookOpen :size="17" />在线阅读</button>
          <button v-if="book.novelId" class="action-button" title="继续下载" @click="emit('continueDownload')"><Download :size="17" />继续下载</button>
          <button class="action-button" :disabled="!book.chapters.length || exporting" title="导出 EPUB" @click="emit('export')"><FileDown :size="17" />{{ exporting ? "正在导出" : "导出 EPUB" }}</button>
          <a v-if="book.audioHref" class="action-button" :href="book.audioHref" target="_blank"><Headphones :size="17" />有声目录</a>
        </div>
      </div>
    </section>
    <section class="chapter-section">
      <div class="chapter-heading"><span><ListTree :size="19" />已下载章节目录</span><small>{{ book.chapters.length }} 章</small></div>
      <div v-if="book.chapters.length" class="chapter-list">
        <button v-for="chapter in book.chapters" :key="chapter.id" class="chapter-row" @click="emit('read', chapter.id)"><span>{{ chapter.volume }}</span><strong>{{ chapter.title }}</strong></button>
      </div>
      <div v-else class="empty-state"><p>尚未保存可阅读的文字章节</p></div>
    </section>
  </section>
</template>

<style scoped>
.detail-page { padding-bottom: 36px; }
.back-button { display: inline-flex; align-items: center; gap: 3px; margin-bottom: 13px; padding: 0; border: 0; color: #94675c; background: none; font-size: 12px; font-weight: 600; }
.book-summary { display: grid; grid-template-columns: 154px minmax(0, 1fr); gap: 24px; padding: 20px; border-radius: 14px; }
.detail-cover { display: grid; aspect-ratio: 3 / 4; overflow: hidden; border-radius: 8px; color: var(--theme-color-dark); background: #ffe7d8; place-items: center; }
.detail-cover img { width: 100%; height: 100%; object-fit: cover; }
.detail-copy { min-width: 0; }
.eyebrow { margin: 0 0 5px; color: #b08072; font: 10px monospace; letter-spacing: .1em; }
h2 { margin: 0; color: #69453d; font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 25px; }
.author { margin: 6px 0 12px; color: #987972; font-size: 13px; }
.description { display: -webkit-box; overflow: hidden; margin: 0; color: #785e57; font-size: 13px; line-height: 1.65; white-space: pre-line; -webkit-box-orient: vertical; -webkit-line-clamp: 3; }
.detail-actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 15px; }
.action-button { display: inline-flex; align-items: center; gap: 5px; min-height: 34px; padding: 0 10px; border: 1px solid #ead1c5; border-radius: 7px; color: #895c51; background: #fffaf7; font-size: 12px; font-weight: 600; text-decoration: none; }
.action-button:hover:not(:disabled) { border-color: var(--theme-color); color: #fff; background: var(--theme-color); }
.action-button:disabled { cursor: not-allowed; opacity: .5; }
.chapter-section { margin-top: 24px; }
.chapter-heading { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; color: #69453d; }
.chapter-heading span { display: flex; align-items: center; gap: 6px; font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 18px; }
.chapter-heading small { color: #ad877a; font-size: 12px; }
.chapter-list { overflow: hidden; border: 1px solid rgba(235, 205, 192, .9); border-radius: 9px; background: rgba(255,255,255,.45); }
.chapter-row { display: grid; grid-template-columns: 150px 1fr; width: 100%; min-height: 43px; padding: 0 14px; border: 0; border-bottom: 1px solid rgba(187, 132, 111, .13); color: #65433c; background: none; text-align: left; }
.chapter-row:last-child { border-bottom: 0; }.chapter-row:hover { background: #fff5ef; }.chapter-row span { overflow: hidden; padding-right: 12px; color: #a47b6d; font-size: 11px; line-height: 43px; text-overflow: ellipsis; white-space: nowrap; }.chapter-row strong { overflow: hidden; font-size: 13px; font-weight: 500; line-height: 43px; text-overflow: ellipsis; white-space: nowrap; }
.empty-state { min-height: 120px; }.empty-state p { margin: 0; }
@media (max-width: 650px) { .book-summary { grid-template-columns: 96px 1fr; gap: 14px; padding: 14px; }.chapter-row { grid-template-columns: 1fr; padding: 8px 12px; }.chapter-row span,.chapter-row strong { line-height: 1.45; }.chapter-row span { padding: 0; }.detail-actions { margin-top: 11px; } }
</style>
