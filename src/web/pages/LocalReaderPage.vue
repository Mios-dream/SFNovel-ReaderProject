<script setup lang="ts">
import { computed } from "vue";
import { ChevronLeft, FileText } from "lucide-vue-next";
import type { LocalChapterContent } from "../types";
const { chapter, bookName } = defineProps<{
  chapter: LocalChapterContent;
  bookName: string;
}>();
const emit = defineEmits<{ back: [] }>();

type ChapterPart =
  | { type: "text"; value: string }
  | { type: "image"; alt: string; src: string };

const chapterParts = computed<ChapterPart[]>(() => {
  const content = chapter.content.replace(/^##\s+.+\r?\n+/, "");
  const pattern = /!\[([^\]]*)\]\((imgs\/[^/)]+)\)/g;
  const parts: ChapterPart[] = [];
  let lastIndex = 0;
  for (const match of content.matchAll(pattern)) {
    const index = match.index || 0;
    if (index > lastIndex)
      parts.push({ type: "text", value: content.slice(lastIndex, index) });
    parts.push({
      type: "image",
      alt: match[1] || "章节插图",
      src: `/library/${encodeURIComponent(bookName)}/${match[2]
        .split("/")
        .map(encodeURIComponent)
        .join("/")}`,
    });
    lastIndex = index + match[0].length;
  }
  if (lastIndex < content.length)
    parts.push({ type: "text", value: content.slice(lastIndex) });
  return parts;
});
</script>

<template>
  <article class="reader">
    <button class="back-button" @click="emit('back')">
      <ChevronLeft :size="17" />{{ bookName }}
    </button>
    <!-- <header>
      <FileText :size="19" /><span>{{ chapter.volume }}</span>
      <h2>{{ chapter.title }}</h2>
    </header> -->
    <div class="chapter-content">
      <template v-for="(part, index) in chapterParts" :key="index">
        <img
          v-if="part.type === 'image'"
          class="chapter-image"
          :src="part.src"
          :alt="part.alt"
        />
        <span v-else class="chapter-text">{{ part.value }}</span>
      </template>
    </div>
  </article>
</template>

<style scoped>
.reader {
  padding: 5px 8px 44px;
  margin: 0 auto;
}
.back-button {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  margin-bottom: 20px;
  padding: 0;
  border: 0;
  color: #94675c;
  background: none;
  font-size: 12px;
  font-weight: 600;
}
header {
  display: flex;
  align-items: center;
  gap: 7px;
  color: #a47567;
}
header span {
  font-size: 12px;
}
h2 {
  margin: 0 0 0 4px;
  color: #69453d;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 23px;
}
.chapter-content {
  margin-top: 25px;
  color: #513e39;
  font-family: "Microsoft YaHei", sans-serif;
  font-size: 16px;
  line-height: 2;
}
.chapter-text {
  white-space: pre-wrap;
}
.chapter-image {
  display: block;
  width: auto;
  max-width: 100%;
  max-height: 80vh;
  margin: 20px auto;
  object-fit: contain;
}
</style>
