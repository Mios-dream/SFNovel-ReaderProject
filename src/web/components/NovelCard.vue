<script setup lang="ts">
import { Download, Headphones } from "lucide-vue-next";
import type { Novel } from "../types";

defineProps<{
  novel: Novel;
  audioAvailable: boolean;
  formatDate: (value: string) => string;
}>();

const emit = defineEmits<{ select: [novel: Novel, mode: "text" | "audio"] }>();
</script>

<template>
  <article
    class="novel-card glass clickable-card"
    role="button"
    tabindex="0"
    @click="emit('select', novel, 'text')"
    @keydown.enter="emit('select', novel, 'text')"
  >
    <img :src="novel.novelCover" :alt="`${novel.novelName} 封面`" />
    <div class="novel-info">
      <p class="novel-name">{{ novel.novelName }}</p>
      <p class="author">{{ novel.authorName }}</p>
      <p class="updated">更新于 {{ formatDate(novel.lastUpdateTime) }}</p>
    </div>
    <div class="novel-actions">
      <button class="download-button" :title="`选择章节并下载：${novel.novelName}`" @click.stop="emit('select', novel, 'text')"><Download :size="18" /></button>
      <button v-if="audioAvailable" class="download-button audio" :title="`选择有声章节并下载：${novel.novelName}`" @click.stop="emit('select', novel, 'audio')"><Headphones :size="17" /></button>
    </div>
  </article>
</template>

<style scoped>
.novel-card { display: grid; grid-template-columns: 70px 1fr 33px; align-items: center; gap: 12px; min-height: 120px; padding: 10px; border-radius: 15px; transition: 0.2s; }
.novel-card:hover { box-shadow: 0 18px 32px rgba(137, 76, 55, 0.16); transform: translateY(-3px); }
.clickable-card { cursor: pointer; }
.clickable-card:focus-visible { outline: 2px solid var(--theme-color); outline-offset: 2px; }
.novel-card img { width: 70px; height: 96px; border-radius: 9px; background: #f3d5c5; object-fit: cover; }
.novel-info { min-width: 0; }
.novel-name { display: -webkit-box; margin: 0; overflow: hidden; color: #63413b; font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 15px; font-weight: 700; line-height: 1.45; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
.author { margin: 7px 0; color: #9a7166; font-size: 12px; }
.updated { margin: 0; color: #bd978a; font-size: 11px; }
.novel-actions { display: grid; gap: 7px; }
.download-button { display: grid; width: 33px; height: 33px; border: 0; border-radius: 10px; color: var(--theme-color-dark); background: #ffeadf; place-items: center; transition: 0.2s; }
.download-button.audio { color: #5c7890; background: #e0eff2; }
.download-button:hover { color: #fff; background: var(--theme-color); transform: rotate(-7deg); }
.download-button.audio:hover { background: #4e8494; }
</style>
