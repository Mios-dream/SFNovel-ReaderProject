<script setup lang="ts">
import { BookOpen, LoaderCircle, Search, Sparkles } from "lucide-vue-next";
import coverImage from "../../../cover.webp";
import NovelCard from "../components/NovelCard.vue";
import type { Novel } from "../types";

defineProps<{ query: string; results: Novel[]; loading: boolean; searched: boolean; formatDate: (value: string) => string }>();
const emit = defineEmits<{ "update:query": [value: string]; search: []; select: [novel: Novel, mode: "text" | "audio"] }>();
</script>

<template>
  <section class="hero glass">
    <div class="hero-copy"><span class="pill"><Sparkles :size="14" />菠萝包轻小说</span><h2>把小说，安静地收进<br />自己的阅读空间。</h2><p>搜索书名，将公开章节或已购章节保存为 Markdown 文件。</p></div>
    <div class="hero-art"><img :src="coverImage" alt="二次元小说封面" /><span class="hero-sticker">今日推荐</span></div>
  </section>

  <form class="search-box glass" @submit.prevent="emit('search')">
    <Search :size="22" />
    <input :value="query" autocomplete="off" placeholder="输入书名、作者或关键词" aria-label="搜索小说" @input="emit('update:query', ($event.target as HTMLInputElement).value)" />
    <button class="primary-button" type="submit" :disabled="loading"><LoaderCircle v-if="loading" class="spin" :size="18" /><span>{{ loading ? '搜索中' : '搜索' }}</span></button>
  </form>

  <section class="section-head"><div><p class="eyebrow">SEARCH RESULTS</p><h2>{{ searched ? `找到 ${results.length} 本作品` : '从一部小说开始' }}</h2></div><p v-if="searched && results.length" class="subtle">点击作品即可加入下载队列</p></section>
  <div v-if="!searched" class="empty-state"><Search :size="28" /><p>输入小说名称，开始搜索</p></div>
  <div v-else-if="!loading && !results.length" class="empty-state"><BookOpen :size="28" /><p>没有找到匹配的作品</p></div>
  <div v-else class="novel-grid">
    <NovelCard v-for="novel in results" :key="novel.novelId" :novel="novel" :format-date="formatDate" @select="(novel, mode) => emit('select', novel, mode)" />
  </div>
</template>

<style scoped>
.hero { position: relative; display: flex; align-items: center; justify-content: space-between; min-height: 242px; padding: 30px 38px; overflow: hidden; border-radius: 21px; background: linear-gradient(105deg, rgba(255, 255, 255, 0.72), rgba(255, 235, 223, 0.64)); }
.hero::after { position: absolute; top: 25px; right: 34%; color: #e7a17e80; content: "✦  ✧  ✦"; font-size: 18px; letter-spacing: 11px; transform: rotate(-9deg); }
.hero-copy { position: relative; z-index: 1; }
.pill { display: inline-flex; align-items: center; gap: 6px; padding: 6px 10px; border: 1px solid #f7cdb7; border-radius: 99px; color: var(--theme-color-dark); background: #ffe8d9; font-size: 11px; }
.hero h2 { margin: 15px 0 9px; color: #5d3d38; font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 31px; line-height: 1.38; text-shadow: 0 2px 0 #fff; }
.hero p { margin: 0; color: #9b746a; font-size: 13px; }
.hero-art { position: relative; display: grid; width: 180px; height: 198px; filter: drop-shadow(0 17px 13px #a75d4435); place-items: center; transform: rotate(5deg); }
.hero-art::before { position: absolute; inset: 5px 14px 0; border-radius: 13px; background: #edb28e; content: ""; transform: rotate(-8deg); }
.hero-art img { position: relative; z-index: 2; width: 130px; height: 174px; border: 4px solid #fff8f4; border-radius: 9px; object-fit: cover; }
.hero-sticker { position: absolute; right: -4px; bottom: 20px; z-index: 3; padding: 7px 10px; border-radius: 6px; color: #fff; background: var(--theme-color-dark); box-shadow: 3px 4px 0 #fff4ee; font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 12px; transform: rotate(-10deg); }
.search-box { display: flex; align-items: center; gap: 11px; margin-top: 18px; padding: 8px 8px 8px 17px; border-radius: 15px; color: #b17f70; }
.search-box input { flex: 1; min-width: 0; border: 0; outline: 0; color: #5f423c; background: transparent; font-size: 14px; }
.search-box input::placeholder { color: #c19a8d; }
.primary-button { display: inline-flex; align-items: center; justify-content: center; gap: 8px; min-height: 39px; padding: 0 16px; border: 0; border-radius: 10px; color: #fff; background: var(--theme-color); box-shadow: 0 7px 12px #e2946440; font-size: 13px; font-weight: 600; transition: 0.2s; }
.primary-button:hover { background: var(--theme-color-dark); transform: translateY(-1px); }
.primary-button:disabled { cursor: wait; opacity: 0.7; }
.section-head { display: flex; align-items: flex-end; justify-content: space-between; margin: 33px 0 15px; }
.eyebrow { margin: 0 0 6px; color: #b08072; font: 10px monospace; letter-spacing: 0.1em; }
.section-head h2 { margin: 0; color: #69453d; font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 20px; }
.subtle { margin: 0; color: #b08c81; font-size: 12px; }
.novel-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
.empty-state { display: grid; min-height: 195px; gap: 9px; border: 1px dashed #e6bca7; border-radius: 15px; color: #b28c81; background: rgba(255, 250, 247, 0.25); font-size: 13px; justify-items: center; place-content: center; }
.empty-state p { margin: 0; }

@media (max-width: 760px) {
  .hero { min-height: 215px; padding: 25px 21px; }
  .hero h2 { font-size: 25px; }
  .hero-art { width: 85px; height: 155px; opacity: 0.92; }
  .hero-art img { width: 88px; height: 124px; }
  .hero-sticker { right: -8px; bottom: 12px; font-size: 10px; }
  .novel-grid { grid-template-columns: 1fr; }
  .section-head { margin-top: 27px; }
  .subtle { display: none; }
}
</style>
