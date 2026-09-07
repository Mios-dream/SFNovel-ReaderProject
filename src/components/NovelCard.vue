<script setup lang="ts">
import {
  Download,
  FileText,
  Headphones,
  Image as ImageIcon,
} from "lucide-vue-next";
import type { Novel } from "../types";

defineProps<{
  novel: Novel;
  formatDate: (value: string) => string;
}>();

// 卡片主体进入详情，右侧操作按钮向父级发出对应的下载请求。
const emit = defineEmits<{
  select: [novel: Novel, mode: "text" | "audio" | "comic"];
  detail: [novel: Novel];
}>();
</script>

<template>
  <article
    class="novel-card glass clickable-card"
    role="button"
    tabindex="0"
    @click="emit('detail', novel)"
    @keydown.enter.self="emit('detail', novel)"
  >
    <img
      v-if="novel.novelCover"
      :src="novel.novelCover"
      :alt="`${novel.novelName} 封面`"
    />
    <div v-else class="novel-cover-placeholder" aria-hidden="true">
      <ImageIcon v-if="novel.bookshelfType === 'comic'" :size="25" />
      <FileText v-else :size="25" />
    </div>
    <div class="novel-info">
      <div style="display: flex; flex-direction: column">
        <p class="novel-name">{{ novel.novelName }}</p>
        <p class="author">{{ novel.authorName }}</p>
      </div>

      <div>
        <p
          class="media-type"
          :class="`media-type-${novel.bookshelfType || 'novel'}`"
        >
          <Headphones v-if="novel.bookshelfType === 'audio'" :size="12" />
          <ImageIcon v-else-if="novel.bookshelfType === 'comic'" :size="12" />
          <FileText v-else :size="12" />
          {{
            novel.bookshelfType === "audio"
              ? "有声"
              : novel.bookshelfType === "comic"
                ? "漫画"
                : "文字"
          }}
        </p>
        <p class="updated">更新于 {{ formatDate(novel.lastUpdateTime) }}</p>
      </div>
    </div>
    <div class="novel-actions">
      <button
        class="download-button"
        :class="{
          audio: novel.bookshelfType === 'audio',
          comic: novel.bookshelfType === 'comic',
        }"
        :title="
          novel.bookshelfType === 'audio'
            ? `选择有声章节并下载：${novel.novelName}`
            : novel.bookshelfType === 'comic'
              ? `选择漫画章节并下载：${novel.novelName}`
              : `选择章节并下载：${novel.novelName}`
        "
        @click.stop="
          emit(
            'select',
            novel,
            novel.bookshelfType === 'audio'
              ? 'audio'
              : novel.bookshelfType === 'comic'
                ? 'comic'
                : 'text',
          )
        "
      >
        <Headphones
          v-if="novel.bookshelfType === 'audio'"
          :size="18"
        /><ImageIcon v-else-if="novel.bookshelfType === 'comic'" :size="18" /><Download v-else :size="18" />
      </button>
    </div>
  </article>
</template>

<style scoped>
.novel-card {
  display: grid;
  grid-template-columns: 70px 1fr 33px;
  align-items: center;
  gap: 12px;
  min-height: 120px;
  padding: 10px;
  border-radius: 15px;
  transition: 0.2s;
}
.novel-card:hover {
  box-shadow: 0 18px 32px rgba(137, 76, 55, 0.16);
  transform: translateY(-3px);
}
.clickable-card {
  cursor: pointer;
}
.clickable-card:focus-visible {
  outline: 2px solid var(--theme-color);
  outline-offset: 2px;
}
.novel-card img {
  width: 70px;
  height: 96px;
  border-radius: 9px;
  background: #f3d5c5;
  object-fit: cover;
}
.novel-info {
  height: 90%;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
}
.novel-name {
  display: -webkit-box;
  margin: 0;
  overflow: hidden;
  color: #63413b;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 15px;
  font-weight: 700;
  line-height: 1.45;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.author {
  /* margin: 7px 0; */
  margin: 0;
  color: #9a7166;
  font-size: 12px;
}
.updated {
  margin: 0;
  color: #bd978a;
  font-size: 11px;
}
.media-type {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  margin: 0 0 4px;
  font-size: 11px;
}
.media-type-novel {
  color: #a66d59;
}
.media-type-audio {
  color: #a6535c;
}
.media-type-comic {
  color: #8b6a9b;
}
.novel-actions {
  display: grid;
  gap: 7px;
}
.download-button {
  display: grid;
  width: 33px;
  height: 33px;
  border: 0;
  border-radius: 10px;
  color: var(--theme-color-dark);
  background: #ffeadf;
  place-items: center;
  transition: 0.2s;
}
.download-button.audio {
  color: #a6535c;
  background: #f8e5e4;
}
.download-button:hover {
  color: #fff;
  background: var(--theme-color);
  transform: rotate(-7deg);
}
.download-button.audio:hover {
  background: #934149;
}
.novel-cover-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 70px;
  height: 96px;
  border-radius: 9px;
  color: #a66d59;
  background: #f3d5c5;
}
.download-button.comic {
  color: #735383;
  background: #eee5f1;
}
.download-button.comic:hover {
  background: #8b6a9b;
}
</style>
