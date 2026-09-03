<script setup lang="ts">
import { inject } from "vue";
import { useRouter } from "vue-router";
import {
  BookOpen,
  ChevronRight,
  FileText,
  Headphones,
  LibraryBig,
  Pause,
  Play,
  Settings2,
  Trash2,
} from "lucide-vue-next";
import PageFrame from "../components/PageFrame.vue";
import { deskInjectionKey } from "../deskContext";
import type { Book } from "../types";

function requireDesk() {
  const desk = inject(deskInjectionKey);
  if (!desk) throw new Error("Desk context is unavailable");
  return desk;
}

const desk = requireDesk();
const router = useRouter();

async function openBook(book: Book) {
  if (await desk.loadLocalBook(book.name)) {
    await router.push(`/library/${encodeURIComponent(book.name)}`);
  }
}

async function openDiscover() {
  desk.navigate("discover");
  await router.push("/discover");
}

// 本地书库同时展示已完成作品和仍在运行的任务，操作通过事件交给父级处理。
</script>

<template>
  <PageFrame>
    <main class="library-page">
      <section class="section-head library-head">
        <div>
          <p class="eyebrow">LOCAL LIBRARY</p>
          <h2>
            {{
              desk.library.value.length
                ? `共 ${desk.library.value.length} 本本地作品`
                : "本地书库"
            }}
          </h2>
        </div>
        <button
          class="manage-button"
          :class="{ active: desk.libraryManaging.value }"
          :disabled="!desk.library.value.length"
          @click="desk.libraryManaging.value = !desk.libraryManaging.value"
        >
          <Settings2 :size="17" /><span>{{
            desk.libraryManaging.value ? "完成管理" : "管理"
          }}</span>
        </button>
      </section>
      <section
        v-if="desk.libraryJobs.value.length"
        class="library-downloads glass"
      >
        <div class="library-downloads-head">
          <strong>下载进度</strong
          ><span>{{ desk.libraryJobs.value.length }} 个任务</span>
        </div>
        <article
          v-for="job in desk.libraryJobs.value"
          :key="job.id"
          class="library-download-row"
        >
          <div class="library-download-title">
            <span>{{ job.title }}</span
            ><small>{{ job.message }}</small>
          </div>
          <div class="progress">
            <i :style="{ width: `${job.progress}%` }"></i>
          </div>
          <button
            v-if="job.status === 'paused'"
            class="text-button"
            title="继续下载"
            @click="desk.resumeJob(job)"
          >
            <Play :size="14" />继续下载</button
          ><button
            v-else-if="job.status === 'downloading' || job.status === 'queued'"
            class="text-button"
            title="暂停下载"
            @click="desk.pauseJob(job)"
          >
            <Pause :size="14" />暂停
          </button>
        </article>
      </section>
      <section
        v-if="desk.library.value.length"
        class="library-grid"
        :class="{ managing: desk.libraryManaging.value }"
      >
        <article
          v-for="book in desk.library.value"
          :key="book.name"
          class="library-card glass"
          :class="{ 'clickable-card': true }"
          role="button"
          tabindex="0"
          @click="openBook(book)"
          @keydown.enter="openBook(book)"
        >
          <span class="library-cover"
            ><img
              v-if="book.cover"
              :src="book.cover"
              :alt="`${book.name} 封面`" /><BookOpen v-else :size="34"
          /></span>
          <button
            v-if="desk.libraryManaging.value"
            class="delete-badge"
            title="删除本地内容"
            @click.stop="desk.deleteBook(book)"
          >
            <Trash2 :size="15" /><span>删除</span>
          </button>
          <div class="library-info">
            <div>
              <strong>{{ book.name }}</strong
              ><small>最近写入 {{ desk.formatDate(book.updatedAt) }}</small>
            </div>
            <div class="format-icons" aria-label="已下载格式">
              <span
                v-if="book.formats.text"
                class="format-text"
                title="已下载文字小说"
                ><FileText :size="15"
              /></span>
              <span
                v-if="book.formats.audio"
                class="format-audio"
                title="已下载有声内容"
                ><Headphones :size="15"
              /></span>
              <span
                v-if="book.formats.comic"
                class="format-comic"
                title="已下载漫画"
                ><BookOpen :size="15"
              /></span>
            </div>
          </div>
        </article>
      </section>
      <div v-else class="empty-state tall">
        <LibraryBig :size="32" />
        <p>书库还是空的</p>
        <button class="text-button" @click="openDiscover">
          去搜索小说 <ChevronRight :size="16" />
        </button>
      </div>
    </main>
  </PageFrame>
</template>

<style scoped>
.library-page {
  min-width: 0;
  height: 100%;
  padding: 18px 42px 28px 2px;
  overflow-y: auto;
  background-color: var(--background-color);
  scrollbar-gutter: stable;
}
.section-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  margin: 33px 0 15px;
}
.library-head {
  margin-top: 7px;
}
.eyebrow {
  margin: 0 0 6px;
  color: #b08072;
  font: 10px monospace;
  letter-spacing: 0.1em;
}
.section-head h2 {
  margin: 0;
  color: #69453d;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 20px;
}
.manage-button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 32px;
  padding: 0 11px;
  border: 1px solid #efd3c5;
  border-radius: 8px;
  color: #94675c;
  background: rgba(255, 250, 247, 0.72);
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
  transition: 0.2s;
}
.manage-button:hover:not(:disabled),
.manage-button.active {
  border-color: #b85f51;
  color: #fff;
  background: #b85f51;
}
.manage-button:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
.library-downloads {
  margin: 0 0 15px;
  padding: 13px 15px;
  border-radius: 14px;
}
.library-downloads-head,
.library-download-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.library-downloads-head {
  justify-content: space-between;
  margin-bottom: 9px;
  color: #80594f;
  font-size: 13px;
}
.library-downloads-head span,
.library-download-title small {
  color: #ae877a;
  font-size: 11px;
}
.library-download-row {
  padding: 8px 0;
  border-top: 1px solid rgba(166, 100, 76, 0.12);
}
.library-download-title {
  display: grid;
  min-width: 150px;
  gap: 2px;
}
.library-download-title span {
  overflow: hidden;
  color: #65433c;
  font-size: 12px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.library-download-row .progress {
  flex: 1;
}
.progress {
  height: 5px;
  overflow: hidden;
  border-radius: 99px;
  background: #f1ded5;
}
.progress i {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--theme-color);
  transition: width 0.35s;
}
.text-button {
  display: flex;
  align-items: center;
  border: 0;
  color: var(--theme-color-dark);
  background: none;
  font-weight: 600;
}
.library-download-row .text-button {
  flex: 0 0 auto;
  gap: 4px;
  font-size: 11px;
}
.library-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 14px;
}
.library-card {
  position: relative;
  display: grid;
  grid-template-columns: 74px 1fr;
  align-items: center;
  gap: 12px;
  min-width: 0;
  padding: 10px;
  border-radius: 15px;
}
.library-grid.managing .library-card {
  border-color: rgba(184, 95, 81, 0.42);
}
.clickable-card {
  cursor: pointer;
}
.clickable-card:focus-visible {
  outline: 2px solid var(--theme-color);
  outline-offset: 2px;
}
.library-cover {
  display: grid;
  width: 74px;
  height: 104px;
  overflow: hidden;
  border-radius: 9px;
  color: var(--theme-color-dark);
  background: #ffe7d8;
  box-shadow: 0 7px 12px rgba(137, 76, 55, 0.1);
  place-items: center;
}
.library-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.library-info {
  height: 90%;
  display: flex;
  flex-direction: column;
  justify-content: space-around;
  min-width: 0;
  gap: 5px;
}
.library-info strong {
  display: -webkit-box;
  overflow: hidden;
  color: #65433c;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 16px;
  line-height: 1.35;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.library-info small {
  color: #b18b7e;
  font-size: 11px;
}
.format-icons {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 3px;
}
.format-icons span {
  display: grid;
  width: 23px;
  height: 23px;
  border: 1px solid #ecd2c5;
  border-radius: 6px;
  place-items: center;
}
.format-icons .format-text {
  border-color: #ecd2c5;
  color: #a66d59;
  background: #fff8f4;
}
.format-icons .format-audio {
  border-color: #e5bec0;
  color: #a6535c;
  background: #fdf1f0;
}
.format-icons .format-comic {
  border-color: #d9c5df;
  color: #8b6a9b;
  background: #faf4fc;
}
.delete-badge {
  position: absolute;
  top: 7px;
  right: 7px;
  z-index: 2;
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 4px 6px;
  border: 0;
  border-radius: 7px;
  color: #fff;
  background: #b85f51;
  box-shadow: 0 4px 9px rgba(137, 76, 55, 0.18);
  font-size: 10px;
  font-weight: 600;
}
.delete-badge:hover {
  background: #8d3127;
}
.empty-state {
  display: grid;
  min-height: 195px;
  gap: 9px;
  border: 1px dashed #e6bca7;
  border-radius: 15px;
  color: #b28c81;
  background: rgba(255, 250, 247, 0.25);
  font-size: 13px;
  justify-items: center;
  place-content: center;
}
.empty-state p {
  margin: 0;
}
.empty-state.tall {
  min-height: 380px;
}

@media (max-width: 1150px) {
  .library-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
@media (max-width: 760px) {
  .library-page {
    height: auto;
    padding: 10px 0 0;
    overflow: visible;
  }
  .section-head {
    margin-top: 20px;
  }
  .library-grid {
    grid-template-columns: 1fr;
  }
  .library-download-row {
    flex-wrap: wrap;
  }
  .library-download-title,
  .library-download-row .progress {
    flex-basis: 100%;
  }
  .library-download-row .text-button {
    min-height: 32px;
    margin-left: auto;
    padding: 0 8px;
    border-radius: 7px;
    background: #fff0e7;
  }
}
</style>
