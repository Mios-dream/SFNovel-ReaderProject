<script setup lang="ts">
import { BookMarked, ChevronLeft, ChevronRight, LoaderCircle, RefreshCw } from "lucide-vue-next";
import NovelCard from "../components/NovelCard.vue";
import type { Novel } from "../types";

defineProps<{ novels: Novel[]; visibleNovels: Novel[]; loading: boolean; categories: string[]; activeCategory: string; page: number; totalPages: number; filteredCount: number; audioAvailable: Record<number, boolean>; formatDate: (value: string) => string }>();
const emit = defineEmits<{ refresh: []; category: [value: string]; page: [value: number]; select: [novel: Novel, mode: "text" | "audio"] }>();
</script>

<template>
  <section class="section-head shelf-head"><div><p class="eyebrow">SFACG BOOKSHELF</p><h2>{{ loading ? '正在同步书架' : `共 ${novels.length} 本收藏` }}</h2></div><button class="icon-button" title="刷新书架" :disabled="loading" @click="emit('refresh')"><RefreshCw :class="{ spin: loading }" :size="19" /></button></section>
  <div v-if="loading && !novels.length" class="empty-state"><LoaderCircle class="spin" :size="28" /><p>正在读取书架</p></div>
  <template v-else-if="novels.length">
    <div class="bookshelf-toolbar" aria-label="书架分类"><button v-for="category in categories" :key="category" class="category-tab" :class="{ active: activeCategory === category }" @click="emit('category', category)">{{ category }}</button></div>
    <div class="novel-grid shelf-grid"><NovelCard v-for="novel in visibleNovels" :key="`${novel.novelId}-${novel.bookshelfName || '默认书架'}`" :novel="novel" :audio-available="Boolean(audioAvailable[novel.novelId])" :format-date="formatDate" @select="(novel, mode) => emit('select', novel, mode)" /></div>
    <nav v-if="totalPages > 1" class="pagination" aria-label="书架分页"><button class="icon-button" title="上一页" :disabled="page === 1" @click="emit('page', page - 1)"><ChevronLeft :size="18" /></button><span>第 {{ page }} / {{ totalPages }} 页 · {{ filteredCount }} 本</span><button class="icon-button" title="下一页" :disabled="page === totalPages" @click="emit('page', page + 1)"><ChevronRight :size="18" /></button></nav>
  </template>
  <div v-else class="empty-state tall"><BookMarked :size="32" /><p>书架中还没有可显示的作品</p></div>
</template>

<style scoped>
.section-head { display: flex; align-items: flex-end; justify-content: space-between; margin: 33px 0 15px; }
.shelf-head { margin-top: 7px; }
.eyebrow { margin: 0 0 6px; color: #b08072; font: 10px monospace; letter-spacing: 0.1em; }
.section-head h2 { margin: 0; color: #69453d; font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 20px; }
.icon-button { display: grid; width: 40px; height: 40px; border: 0; border-radius: 13px; color: #a17367; background: rgba(255, 255, 255, 0.7); place-items: center; transition: 0.2s; }
.icon-button:hover { color: var(--theme-color-dark); background: #fff0e7; }
.empty-state { display: grid; min-height: 195px; gap: 9px; border: 1px dashed #e6bca7; border-radius: 15px; color: #b28c81; background: rgba(255, 250, 247, 0.25); font-size: 13px; justify-items: center; place-content: center; }
.empty-state p { margin: 0; }
.empty-state.tall { min-height: 380px; }
.bookshelf-toolbar { display: flex; gap: 7px; padding: 2px 1px 12px; overflow-x: auto; scrollbar-width: thin; }
.category-tab { min-height: 32px; padding: 0 11px; border: 1px solid #efd3c5; border-radius: 8px; color: #94675c; background: rgba(255, 250, 247, 0.72); font-size: 12px; font-weight: 600; white-space: nowrap; transition: 0.2s; }
.category-tab:hover, .category-tab.active { border-color: var(--theme-color); color: #fff; background: var(--theme-color); }
.novel-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
.pagination { display: flex; align-items: center; justify-content: center; gap: 10px; margin: 22px 0 4px; color: #a27a6f; font-size: 12px; }
.pagination .icon-button { width: 32px; height: 32px; }
.pagination .icon-button:disabled { cursor: not-allowed; opacity: 0.4; }

@media (max-width: 760px) {
  .novel-grid { grid-template-columns: 1fr; }
  .section-head { margin-top: 27px; }
}
</style>
