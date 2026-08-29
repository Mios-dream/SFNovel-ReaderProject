<script setup lang="ts">
import { ChevronLeft, FileText } from "lucide-vue-next";
import type { LocalChapterContent } from "../types";
defineProps<{ chapter: LocalChapterContent; bookName: string }>();
const emit = defineEmits<{ back: [] }>();
</script>

<template>
  <article class="reader">
    <button class="back-button" @click="emit('back')">
      <ChevronLeft :size="17" />{{ bookName }}
    </button>
    <header>
      <FileText :size="19" /><span>{{ chapter.volume }}</span>
      <h2>{{ chapter.title }}</h2>
    </header>
    <div class="chapter-content">
      {{ chapter.content.replace(/^##\s+.+\r?\n+/, "") }}
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
  white-space: pre-wrap;
}
</style>
