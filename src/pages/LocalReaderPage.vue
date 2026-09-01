<script setup lang="ts">
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { computed } from "vue";
import { ChevronLeft } from "lucide-vue-next";
import type { LocalChapterContent } from "../types";
const { chapter, bookName, imageDirectory } = defineProps<{
  chapter: LocalChapterContent;
  bookName: string;
  imageDirectory: string;
}>();
const emit = defineEmits<{ back: [] }>();

type ChapterPart =
  | { type: "text"; value: string }
  | { type: "image"; alt: string; src: string };

function chapterImageSource(relativePath: string) {
  const fileName = relativePath.slice("imgs/".length).replace(/\\/g, "/");
  const separator = imageDirectory.includes("\\") ? "\\" : "/";
  const imagePath = `${imageDirectory.replace(/[\\/]+$/, "")}${separator}${fileName.replace(/\//g, separator)}`;
  return isTauri() ? convertFileSrc(imagePath) : imagePath;
}

const chapterParts = computed<ChapterPart[]>(() => {
  const content = chapter.content.replace(/^##\s+.+\r?\n+/, "");
  const pattern = /!\[([^\]]*)\]\((imgs[\\/][^)]+)\)/g;
  const parts: ChapterPart[] = [];
  let lastIndex = 0;
  for (const match of content.matchAll(pattern)) {
    const index = match.index || 0;
    if (index > lastIndex)
      parts.push({ type: "text", value: content.slice(lastIndex, index) });
    parts.push({
      type: "image",
      alt: match[1] || "章节插图",
      src: chapterImageSource(match[2]),
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
  width: min(820px, 100%);
  padding: 10px 24px 64px;
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
  padding: 30px clamp(18px, 5vw, 58px);
  margin-top: 0;
  border: 1px solid rgba(224, 176, 154, 0.56);
  border-radius: 8px;
  background: rgba(255, 250, 247, 0.76);
  box-shadow: 0 12px 28px rgba(137, 76, 55, 0.08);
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
@media (max-width: 760px) {
  .reader {
    padding: 14px 16px calc(28px + env(safe-area-inset-bottom));
  }
  .back-button {
    min-height: 40px;
    margin-bottom: 12px;
  }
  .chapter-content {
    padding: 0px;
    background: none;
    border: none;
    box-shadow: none;
    border-radius: none;
  }
  .chapter-image {
    max-height: none;
    margin: 18px auto;
  }
}
</style>
