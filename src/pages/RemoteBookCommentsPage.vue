<script setup lang="ts">
import { computed, inject, nextTick, ref, watch } from "vue";
import { ChevronLeft, LoaderCircle, MessageCircle, Star } from "@lucide/vue";
import { useRoute, useRouter } from "vue-router";
import BookCommentCard from "../components/BookCommentCard.vue";
import DiscussionDanmaku from "../components/DiscussionDanmaku.vue";
import DiscussionTabs from "../components/DiscussionTabs.vue";
import {
  useBookDiscussions,
  type DiscussionTab,
} from "../composables/useBookDiscussions";
import { deskInjectionKey } from "../deskContext";

function requireDesk() {
  const desk = inject(deskInjectionKey);
  if (!desk) throw new Error("Desk context is unavailable");
  return desk;
}

const desk = requireDesk();
const route = useRoute();
const router = useRouter();
const novel = computed(() => desk.remoteBook.value);
const initialTab: DiscussionTab =
  route.query.tab === "reviews" ? "reviews" : "comments";
const { activeTab, current, highlights, selectTab, loadMore, reload } =
  useBookDiscussions(novel, 16, initialTab);
const pageElement = ref<HTMLElement>();

function switchTab(tab: DiscussionTab) {
  if (tab === activeTab.value) return;
  selectTab(tab);
  void router.replace({ path: "/remote-book/comments", query: { tab } });
  void nextTick(() => {
    pageElement.value?.scrollTo({ top: 0, behavior: "smooth" });
  });
}

function handleScroll() {
  const element = pageElement.value;
  if (!element || current.value.loading || current.value.loadingMore) return;
  if (element.scrollHeight - element.scrollTop - element.clientHeight < 240)
    loadMore();
}

function goBack() {
  void router.push(novel.value ? "/remote-book" : "/discover");
}

watch(
  () => route.query.tab,
  (value) => {
    const tab: DiscussionTab = value === "reviews" ? "reviews" : "comments";
    if (tab !== activeTab.value) selectTab(tab);
  },
);
</script>

<template>
  <main ref="pageElement" class="feedback-page" @scroll.passive="handleScroll">
    <header class="feedback-topbar">
      <button class="back-button" title="返回作品详情" @click="goBack">
        <ChevronLeft :size="21" /><span>返回</span>
      </button>
      <span v-if="novel" class="topbar-title">{{ novel.novelName }}</span>
    </header>

    <div v-if="novel" class="feedback-layout">
      <aside class="book-context">
        <div class="context-cover" :class="{ empty: !novel.novelCover }">
          <img
            v-if="novel.novelCover"
            :src="novel.novelCover"
            :alt="`${novel.novelName} 封面`"
          />
          <MessageCircle v-else :size="34" />
        </div>
        <h1>{{ novel.novelName }}</h1>
        <p>{{ novel.authorName || "未知作者" }}</p>
        <div class="context-meta">
          <span v-if="novel.score != null"
            ><Star :size="13" fill="currentColor" />{{
              novel.score.toFixed(1)
            }}</span
          >
          <span v-if="novel.typeName || novel.bookshelfName">{{
            novel.typeName || novel.bookshelfName
          }}</span>
        </div>
      </aside>

      <section class="feedback-main" aria-labelledby="feedback-tabs">
        <DiscussionDanmaku :items="highlights" />
        <div class="feedback-toolbar">
          <DiscussionTabs
            id="feedback-tabs"
            :model-value="activeTab"
            @update:model-value="switchTab"
          />
        </div>

        <div v-if="current.loading" class="feedback-state">
          <LoaderCircle class="spin" :size="24" />
          {{ activeTab === "reviews" ? "正在读取书评" : "正在读取评论" }}
        </div>
        <div
          v-else-if="current.error && !current.items.length"
          class="feedback-state error"
        >
          <span>{{ current.error }}</span>
          <button @click="reload">重试</button>
        </div>
        <div v-else-if="!current.items.length" class="feedback-state">
          <MessageCircle :size="24" />
          {{ activeTab === "reviews" ? "暂无书评" : "暂无评论" }}
        </div>
        <template v-else>
          <div class="feedback-list">
            <BookCommentCard
              v-for="comment in current.items"
              :key="comment.commentId"
              :comment="comment"
            />
          </div>
          <footer class="feedback-footer">
            <div v-if="current.loadingMore" class="feedback-loading">
              <LoaderCircle class="spin" :size="17" />加载中
            </div>
            <button
              v-else-if="current.error"
              class="footer-retry"
              @click="loadMore"
            >
              加载失败，点击重试
            </button>
            <span v-else-if="!current.hasMore">没有更多了</span>
          </footer>
        </template>
      </section>
    </div>

    <div v-else class="feedback-state missing">
      <MessageCircle :size="26" />未找到远程作品
    </div>
  </main>
</template>

<style scoped>
.feedback-page {
  height: 100dvh;
  overflow-y: auto;
  color: #493e37;
  background: var(--background-color);
}
.feedback-topbar {
  position: sticky;
  top: 0;
  z-index: 5;
  display: flex;
  min-height: 65px;
  align-items: center;
  gap: 14px;
  padding: 12px max(24px, calc((100vw - 1060px) / 2));
  border-bottom: 1px solid rgba(232, 220, 208, 0.72);
  background: rgba(252, 237, 230, 0.9);
  backdrop-filter: blur(16px);
}
.back-button {
  display: inline-flex;
  min-height: 38px;
  align-items: center;
  gap: 3px;
  padding: 0 14px 0 9px;
  border: 1px solid #eaded2;
  border-radius: 12px;
  color: #765f54;
  background: #fffdf8;
  font: inherit;
  font-size: 12px;
}
.back-button:hover {
  border-color: #e3b099;
  color: #b4573b;
}
.topbar-title {
  min-width: 0;
  overflow: hidden;
  color: #79675c;
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.feedback-layout {
  display: grid;
  width: min(1060px, calc(100% - 48px));
  grid-template-columns: 218px minmax(0, 1fr);
  gap: 38px;
  align-items: start;
  margin: 0 auto;
  padding: 38px 0 70px;
}
.book-context {
  position: sticky;
  top: 103px;
}
.context-cover {
  display: grid;
  width: 100%;
  aspect-ratio: 3 / 4;
  overflow: hidden;
  border-radius: 16px;
  color: #a46a54;
  background: #f3dfd1;
  box-shadow: 0 14px 32px rgba(101, 71, 52, 0.16);
  place-items: center;
}
.context-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.book-context h1 {
  display: -webkit-box;
  overflow: hidden;
  margin: 18px 0 0;
  color: #493d35;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 22px;
  line-height: 1.35;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.book-context p {
  margin: 7px 0 0;
  color: #8e7c70;
  font-size: 13px;
}
.context-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  margin-top: 14px;
}
.context-meta span {
  display: inline-flex;
  min-height: 26px;
  align-items: center;
  gap: 4px;
  padding: 0 9px;
  border-radius: 999px;
  color: #8a7264;
  background: #f2e9e1;
  font-size: 12px;
}
.feedback-main {
  min-width: 0;
  padding: 21px 24px 16px;
  border: 1px solid #eadfd3;
  border-radius: 20px;
  background: #fffdf9;
  box-shadow: 0 13px 34px rgba(91, 66, 50, 0.08);
}
.feedback-toolbar {
  position: sticky;
  top: 65px;
  z-index: 2;
  padding: 0 0 14px;
  background: #fffdf9;
}
.feedback-list {
  border-top: 1px solid #eee5db;
}
.feedback-list :deep(.book-comment + .book-comment) {
  border-top: 1px solid #f0e8df;
}
.feedback-state {
  display: flex;
  min-height: 300px;
  align-items: center;
  justify-content: center;
  gap: 9px;
  color: #9b8578;
  font-size: 13px;
}
.feedback-state.error {
  flex-direction: column;
  color: #ad6654;
  text-align: center;
}
.feedback-state.missing {
  min-height: 60dvh;
}
.feedback-state button,
.footer-retry {
  min-height: 32px;
  padding: 0 12px;
  border: 1px solid #e7c6b5;
  border-radius: 8px;
  color: #a75b40;
  background: #fffaf6;
  font: inherit;
  font-size: 12px;
}
.feedback-footer {
  display: flex;
  min-height: 47px;
  align-items: center;
  justify-content: center;
  border-top: 1px solid #f0e8df;
  color: #aa978b;
  font-size: 12px;
}
.feedback-loading {
  display: inline-flex;
  align-items: center;
  gap: 7px;
}
@media (max-width: 900px) {
  .feedback-page {
    background: #fff;
  }
  .feedback-topbar {
    min-height: 59px;
    gap: 10px;
    padding: 9px 14px;
    background: #fff;
    backdrop-filter: none;
  }
  .back-button {
    width: 40px;
    height: 40px;
    min-height: 40px;
    justify-content: center;
    padding: 0;
    border: 0;
    border-radius: 50%;
    color: #574a41;
    background: #f5f3f0;
  }
  .back-button span {
    display: none;
  }
  .topbar-title {
    color: #6b5d54;
    font-size: 14px;
    font-weight: 600;
  }
  .feedback-layout {
    display: block;
    width: 100%;
    padding: 0 16px calc(34px + env(safe-area-inset-bottom));
  }
  .book-context {
    display: none;
  }
  .feedback-main {
    padding: 13px 0 0;
    border: 0;
    border-radius: 0;
    background: #fff;
    box-shadow: none;
  }
  .feedback-toolbar {
    top: 59px;
    padding-bottom: 10px;
    background: #fff;
  }
  .feedback-state {
    min-height: 260px;
  }
}
@media (prefers-reduced-motion: reduce) {
  .back-button {
    transition: none;
  }
}
</style>
