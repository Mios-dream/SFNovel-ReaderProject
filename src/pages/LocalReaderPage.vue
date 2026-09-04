<script setup lang="ts">
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { computed, inject, onMounted, ref } from "vue";
import { ChevronLeft, Images, ListTree, X } from "lucide-vue-next";
import { useRouter } from "vue-router";
import { deskInjectionKey } from "../deskContext";

function requireDesk() {
  const desk = inject(deskInjectionKey);
  if (!desk) throw new Error("Desk context is unavailable");
  return desk;
}

const desk = requireDesk();
const router = useRouter();
const chapter = computed(() => desk.localChapter.value);
const comicChapter = computed(() => desk.localComicChapter.value);
const bookName = computed(() => desk.localBook.value?.name || "本地阅读");
const imageDirectory = computed(
  () => desk.localBook.value?.imageDirectory || "",
);
const comicChapters = computed(() => desk.localBook.value?.comicChapters || []);
const comicDirectoryOpen = ref(false);

type ChapterPart =
  | { type: "text"; value: string }
  | { type: "image"; alt: string; src: string };

function chapterImageSource(relativePath: string) {
  const fileName = relativePath.slice("imgs/".length).replace(/\\/g, "/");
  const separator = imageDirectory.value.includes("\\") ? "\\" : "/";
  const imagePath = `${imageDirectory.value.replace(/[\\/]+$/, "")}${separator}${fileName.replace(/\//g, separator)}`;
  return isTauri() ? convertFileSrc(imagePath) : imagePath;
}

const chapterParts = computed<ChapterPart[]>(() => {
  const content = chapter.value?.content.replace(/^##\s+.+\r?\n+/, "") || "";
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

onMounted(() => {
  if (!desk.localBook.value || (!desk.localChapter.value && !desk.localComicChapter.value)) {
    void router.replace("/library");
    return;
  }
  desk.navigate("reader");
});

async function openComicChapter(chapterId: number) {
  await desk.openLocalComicChapter(chapterId);
  comicDirectoryOpen.value = false;
}
</script>

<template>
  <article v-if="chapter || comicChapter" class="reader">
    <header class="reader-toolbar">
      <button
        class="back-button"
        @click="router.push(`/library/${encodeURIComponent(bookName)}`)"
      >
        <ChevronLeft :size="17" />{{ bookName }}
      </button>
      <button
        v-if="comicChapter"
        class="comic-directory-button"
        title="漫画目录"
        @click="comicDirectoryOpen = true"
      >
        <ListTree :size="18" />
      </button>
    </header>
    <!-- <header>
      <FileText :size="19" /><span>{{ chapter.volume }}</span>
      <h2>{{ chapter.title }}</h2>
    </header> -->
    <div v-if="chapter" class="chapter-content">
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
    <div v-else class="comic-reader-content">
      <h1>{{ comicChapter?.title }}</h1>
      <img v-for="(page, index) in comicChapter?.pages" :key="page" class="comic-page-image" :src="page" :alt="`第 ${index + 1} 页`" />
    </div>
    <div
      v-if="comicDirectoryOpen"
      class="comic-directory-backdrop"
      @click.self="comicDirectoryOpen = false"
    >
      <section class="comic-directory-panel" role="dialog" aria-modal="true" aria-label="漫画目录">
        <header>
          <div><small>本地漫画</small><h2>章节目录</h2></div>
          <button title="关闭目录" @click="comicDirectoryOpen = false"><X :size="19" /></button>
        </header>
        <button
          v-for="chapterItem in comicChapters"
          :key="chapterItem.id"
          class="comic-directory-item"
          :class="{ active: chapterItem.id === comicChapter?.id }"
          @click="openComicChapter(chapterItem.id)"
        >
          <img v-if="chapterItem.cover" :src="chapterItem.cover" :alt="`${chapterItem.title} 首图`" />
          <Images v-else :size="26" />
          <strong>{{ chapterItem.title }}</strong>
        </button>
      </section>
    </div>
  </article>
</template>

<style scoped>
.reader {
  width: min(820px, 100%);
  min-height: 100dvh;
  overflow-y: auto;
  padding: 10px 24px 64px;
  margin: 0 auto;
  background: #f8f6f0;
}
.reader-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
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
.comic-directory-button {
  display: inline-flex;
  width: 36px;
  height: 36px;
  align-items: center;
  justify-content: center;
  border: 1px solid #e7d8c9;
  border-radius: 6px;
  color: #805f67;
  background: #fffaf5;
}
.comic-reader-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 14px;
  padding-bottom: 40px;
}
.comic-reader-content h1 {
  align-self: stretch;
  margin: 12px 0;
  color: #69453d;
  font-size: 21px;
}
.comic-page-image {
  display: block;
  width: min(100%, 760px);
  height: auto;
  object-fit: contain;
}
.comic-directory-backdrop {
  position: fixed;
  z-index: 20;
  inset: 0;
  display: flex;
  justify-content: flex-end;
  background: rgba(42, 34, 37, 0.45);
}
.comic-directory-panel {
  display: flex;
  width: min(390px, 100%);
  height: 100%;
  flex-direction: column;
  overflow-y: auto;
  background: #fffdf8;
  box-shadow: -12px 0 36px rgba(54, 37, 39, 0.22);
}
.comic-directory-panel > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 22px 20px 15px;
  border-bottom: 1px solid #eee5da;
}
.comic-directory-panel small {
  color: #aa8b78;
  font-size: 11px;
}
.comic-directory-panel h2 {
  margin: 4px 0 0;
  color: #5d4541;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 21px;
}
.comic-directory-panel header button {
  display: inline-flex;
  width: 36px;
  height: 36px;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 6px;
  color: #765c57;
  background: #f7eee5;
}
.comic-directory-item {
  display: flex;
  min-height: 78px;
  align-items: center;
  gap: 12px;
  padding: 9px 20px;
  border: 0;
  border-bottom: 1px solid #f0eae1;
  color: #4e403c;
  background: transparent;
  text-align: left;
}
.comic-directory-item:hover,
.comic-directory-item.active {
  background: #fff4ea;
}
.comic-directory-item img {
  width: 72px;
  height: 48px;
  flex: 0 0 auto;
  border-radius: 4px;
  object-fit: cover;
}
.comic-directory-item strong {
  min-width: 0;
  overflow: hidden;
  font-size: 14px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
@media (max-width: 760px) {
  .reader {
    min-height: 100dvh;
    padding: 14px 16px calc(28px + env(safe-area-inset-bottom));
  }
  .back-button {
    min-height: 40px;
    margin-bottom: 12px;
  }
  .comic-directory-panel {
    width: min(100%, 420px);
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
