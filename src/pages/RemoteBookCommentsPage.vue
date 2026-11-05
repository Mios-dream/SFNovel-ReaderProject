<script setup lang="ts">
import { computed, inject, nextTick, ref, watch } from "vue";
import {
  ArrowUpRight,
  BookOpen,
  ChevronLeft,
  LoaderCircle,
  MessageCircle,
  Star,
} from "@lucide/vue";
import { useRoute, useRouter } from "vue-router";
import BookCommentCard from "../components/BookCommentCard.vue";
import BookCommentDetail from "../components/BookCommentDetail.vue";
import DiscussionDanmaku from "../components/DiscussionDanmaku.vue";
import DiscussionTabs from "../components/DiscussionTabs.vue";
import {
  useBookDiscussions,
  type DiscussionTab,
} from "../composables/useBookDiscussions";
import { deskInjectionKey } from "../deskContext";
import type { BookComment } from "../types";

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
const selectedComment = ref<BookComment>();

const mediaKind = computed<"novel" | "audio" | "comic">(
  () => novel.value?.bookshelfType || "novel",
);
const mediaLabel = computed(() =>
  mediaKind.value === "audio"
    ? "有声"
    : mediaKind.value === "comic"
      ? "漫画"
      : "小说",
);
const tabLabel = computed(() =>
  activeTab.value === "reviews" ? "书评" : "评论",
);
const contextTags = computed(() => {
  const work = novel.value;
  if (!work) return [];
  return [
    work.typeName || work.bookshelfName,
    work.categoryName,
    work.isFinish ? "已完结" : "连载中",
    ...(work.tags || []),
  ]
    .filter((tag): tag is string => Boolean(tag))
    .filter((tag, index, tags) => tags.indexOf(tag) === index)
    .slice(0, 5);
});
const latestChapter = computed(
  () =>
    novel.value?.latestChapterTitle ||
    novel.value?.latestChapterTime ||
    novel.value?.lastUpdateTime,
);

function formatMetric(value?: number) {
  if (value == null) return "--";
  if (value >= 10000)
    return `${(value / 10000).toFixed(value >= 100000 ? 0 : 1)}万`;
  return value.toLocaleString("zh-CN");
}

function formatDate(value?: string) {
  if (!value) return "";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : new Intl.DateTimeFormat("zh-CN", {
        year: "numeric",
        month: "numeric",
        day: "numeric",
      }).format(date);
}

function switchTab(tab: DiscussionTab) {
  if (tab === activeTab.value) return;
  selectedComment.value = undefined;
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

function goDetail() {
  if (novel.value) void router.push("/remote-book");
}

function openComment(comment: BookComment) {
  selectedComment.value = comment;
}

function closeComment() {
  selectedComment.value = undefined;
  if (route.query.comment)
    void router.replace({
      path: "/remote-book/comments",
      query: { tab: activeTab.value },
    });
}

watch(
  () => route.query.tab,
  (value) => {
    const tab: DiscussionTab = value === "reviews" ? "reviews" : "comments";
    if (tab !== activeTab.value) {
      selectedComment.value = undefined;
      selectTab(tab);
    }
  },
);

watch(
  () => [current.value.items, route.query.comment] as const,
  () => {
    const commentId = Number(route.query.comment);
    if (!commentId || selectedComment.value?.commentId === commentId) return;
    const comment = current.value.items.find(
      (item) => item.commentId === commentId,
    );
    if (comment) selectedComment.value = comment;
  },
  { immediate: true },
);
</script>

<template>
  <main ref="pageElement" class="feedback-page" @scroll.passive="handleScroll">
    <header class="feedback-topbar">
      <div class="topbar-inner">
        <button class="back-button" title="返回作品详情" @click="goBack">
          <ChevronLeft :size="20" /><span>返回</span>
        </button>
        <nav class="topbar-path" aria-label="当前位置">
          <span v-if="novel" class="topbar-book">{{ novel.novelName }}</span>
        </nav>
      </div>
    </header>

    <div v-if="novel" class="feedback-layout">
      <aside class="book-context">
        <section class="context-card">
          <div class="context-cover-wrap">
            <div
              class="context-cover-backdrop"
              :style="
                novel.novelCover
                  ? { backgroundImage: `url(${novel.novelCover})` }
                  : undefined
              "
            />
            <div class="context-cover" :class="{ empty: !novel.novelCover }">
              <img
                v-if="novel.novelCover"
                :src="novel.novelCover"
                :alt="`${novel.novelName} 封面`"
              />
              <BookOpen v-else :size="34" />
            </div>
            <span class="context-format">{{ mediaLabel }}</span>
            <span v-if="novel.score != null" class="context-score">
              <Star :size="13" fill="currentColor" />{{
                novel.score.toFixed(1)
              }}
            </span>
          </div>

          <div class="context-copy">
            <h1>{{ novel.novelName }}</h1>
            <p class="context-author">{{ novel.authorName || "未知作者" }}</p>
            <div class="context-tags">
              <span v-for="tag in contextTags" :key="tag">{{ tag }}</span>
            </div>
            <div class="context-stats">
              <div>
                <strong>{{ formatMetric(novel.chapterCount) }}</strong>
                <span>章节</span>
              </div>
              <div>
                <strong>{{ formatMetric(novel.characterCount) }}</strong>
                <span>字数</span>
              </div>
              <div>
                <strong>{{ formatMetric(novel.markCount) }}</strong>
                <span>收藏</span>
              </div>
            </div>
            <button class="context-detail" @click="goDetail">
              <span>查看作品详情</span><ArrowUpRight :size="15" />
            </button>
          </div>
        </section>

        <section v-if="latestChapter" class="context-update">
          <span>最近更新</span>
          <p>{{ latestChapter }}</p>
          <time>{{
            formatDate(novel.latestChapterTime || novel.lastUpdateTime)
          }}</time>
        </section>
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

        <div
          v-if="current.loading"
          class="feedback-skeletons"
          aria-label="正在读取讨论"
        >
          <div v-for="index in 4" :key="index" class="skeleton-card">
            <span class="skeleton-avatar" />
            <div>
              <span class="skeleton-line short" />
              <span class="skeleton-line" />
              <span class="skeleton-line medium" />
            </div>
          </div>
        </div>
        <div
          v-else-if="current.error && !current.items.length"
          class="feedback-state error"
        >
          <div class="state-icon"><MessageCircle :size="25" /></div>
          <strong>{{ tabLabel }}加载失败</strong>
          <span>{{ current.error }}</span>
          <button @click="reload">重试</button>
        </div>
        <div v-else-if="!current.items.length" class="feedback-state">
          <div class="state-icon"><MessageCircle :size="26" /></div>
          <strong>暂无{{ tabLabel }}</strong>
        </div>
        <template v-else>
          <div
            class="feedback-list"
            :class="{ reviews: activeTab === 'reviews' }"
          >
            <BookCommentCard
              v-for="comment in current.items"
              :key="comment.commentId"
              :comment="comment"
              interactive
              :selected="selectedComment?.commentId === comment.commentId"
              @select="openComment"
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
      <div class="state-icon"><MessageCircle :size="26" /></div>
      <strong>未找到远程作品</strong>
    </div>

    <BookCommentDetail
      v-if="selectedComment"
      :comment="selectedComment"
      :kind="mediaKind"
      :reviews="activeTab === 'reviews'"
      @close="closeComment"
    />
  </main>
</template>

<style scoped>
.feedback-page {
  --surface: #fffdfa;
  --surface-soft: rgba(255, 250, 246, 0.72);
  --line: #eadfd5;
  --text: #4d3f37;
  --muted: #958075;
  position: relative;
  height: 100dvh;
  overflow-y: auto;
  color: var(--text);
  background:
    radial-gradient(
      circle at 91% 2%,
      rgba(226, 148, 100, 0.16),
      transparent 31%
    ),
    radial-gradient(
      circle at 18% 88%,
      rgba(238, 184, 151, 0.12),
      transparent 28%
    ),
    linear-gradient(150deg, #fdf3ed 0%, #faece4 55%, #f9eee8 100%);
}
.feedback-topbar {
  position: sticky;
  top: 0;
  z-index: 5;
  min-height: 66px;
  border-bottom: 1px solid rgba(230, 216, 205, 0.78);
  background: rgba(253, 246, 241, 0.88);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
}
.topbar-inner {
  display: flex;
  width: min(1180px, calc(100% - 56px));
  min-height: 66px;
  align-items: center;
  gap: 16px;
  margin: 0 auto;
}
.back-button {
  display: inline-flex;
  min-height: 38px;
  flex: none;
  align-items: center;
  gap: 3px;
  padding: 0 13px 0 8px;
  border: 1px solid #e8d9ce;
  border-radius: 12px;
  color: #735f54;
  background: rgba(255, 253, 250, 0.86);
  font: inherit;
  font-size: 12px;
  transition:
    border-color 160ms ease,
    color 160ms ease,
    background-color 160ms ease;
}
.back-button:hover {
  border-color: #ddb29c;
  color: #a9573c;
  background: #fffaf6;
}
.topbar-path {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 7px;
  color: #a28e82;
  font-size: 16px;
}
.topbar-path svg {
  flex: none;
  color: #c5b3a7;
}
.topbar-path span,
.topbar-path strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.topbar-book {
  max-width: 260px;
  color: #7d675b;
}
.topbar-path strong {
  color: #5c493f;
  font-weight: 600;
}
.topbar-actions {
  display: flex;
  flex: none;
  align-items: center;
  gap: 10px;
  margin-left: auto;
}
.detail-link {
  display: inline-flex;
  min-height: 38px;
  align-items: center;
  gap: 6px;
  padding: 0 12px;
  border: 1px solid #e6d4c7;
  border-radius: 12px;
  color: #69564b;
  background: rgba(255, 253, 250, 0.82);
  font: inherit;
  font-size: 12px;
  transition:
    border-color 160ms ease,
    color 160ms ease,
    background-color 160ms ease;
}
.detail-link:hover {
  border-color: #ddb29c;
  color: #a9573c;
  background: #fffaf6;
}
.feedback-layout {
  display: grid;
  width: min(1180px, calc(100% - 56px));
  grid-template-columns: 270px minmax(0, 1fr);
  gap: 28px;
  align-items: start;
  margin: 0 auto;
  padding: 30px 0 70px;
}
.book-context {
  position: sticky;
  top: 96px;
  display: grid;
  gap: 12px;
}
.context-card {
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.84);
  border-radius: 22px;
  background: rgba(255, 252, 248, 0.68);
  box-shadow: 0 14px 38px rgba(94, 62, 44, 0.1);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
}
.context-cover-wrap {
  position: relative;
  margin: 14px 14px 0;
  overflow: hidden;
  border-radius: 16px;
  background: #f1dfd3;
}
.context-cover-backdrop {
  position: absolute;
  inset: -18px;
  background-position: center;
  background-size: cover;
  filter: blur(20px) saturate(0.85);
  opacity: 0.55;
  transform: scale(1.12);
}
.context-cover-wrap::after {
  position: absolute;
  inset: 0;
  background: linear-gradient(180deg, transparent 56%, rgba(52, 33, 24, 0.28));
  content: "";
  pointer-events: none;
}
.context-cover {
  position: relative;
  display: grid;
  width: 100%;
  aspect-ratio: 3 / 4;
  overflow: hidden;
  color: #a46a54;
  place-items: center;
}
.context-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.context-format,
.context-score {
  position: absolute;
  z-index: 1;
  bottom: 10px;
  display: inline-flex;
  min-height: 27px;
  align-items: center;
  gap: 4px;
  padding: 0 9px;
  border: 1px solid rgba(255, 255, 255, 0.26);
  border-radius: 8px;
  color: #fff;
  background: rgba(50, 34, 26, 0.46);
  backdrop-filter: blur(8px);
  font-size: 12px;
}
.context-format {
  left: 10px;
}
.context-score {
  right: 10px;
  color: #fff5eb;
}
.context-copy {
  padding: 18px 16px 16px;
}
.context-copy h1 {
  display: -webkit-box;
  overflow: hidden;
  margin: 0;
  color: #493a32;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 22px;
  line-height: 1.35;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.context-author {
  margin: 7px 0 0;
  color: #8d776b;
  font-size: 13px;
}
.context-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 14px;
}
.context-tags span {
  padding: 4px 8px;
  border: 1px solid rgba(230, 212, 199, 0.72);
  border-radius: 7px;
  color: #8a7164;
  background: rgba(250, 243, 237, 0.76);
  font-size: 12px;
  line-height: 1.2;
}
.context-stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 4px;
  margin-top: 18px;
  padding: 14px 0;
  border-top: 1px solid rgba(234, 222, 212, 0.78);
  border-bottom: 1px solid rgba(234, 222, 212, 0.78);
}
.context-stats div {
  display: flex;
  min-width: 0;
  flex-direction: column;
  align-items: center;
  gap: 3px;
}
.context-stats strong {
  color: #5a463c;
  font-size: 15px;
  font-weight: 600;
}
.context-stats span {
  color: #a18e83;
  font-size: 12px;
}
.context-detail {
  display: flex;
  width: 100%;
  min-height: 40px;
  align-items: center;
  justify-content: space-between;
  margin-top: 14px;
  padding: 0 13px;
  border: 1px solid #e8d5c8;
  border-radius: 11px;
  color: #8c5845;
  background: rgba(255, 249, 244, 0.8);
  font: inherit;
  font-size: 12px;
  transition:
    border-color 160ms ease,
    background-color 160ms ease;
}
.context-detail:hover {
  border-color: #dfb49d;
  background: #fff6ef;
}
.context-update {
  padding: 15px 17px;
  border: 1px solid rgba(255, 255, 255, 0.78);
  border-radius: 16px;
  background: rgba(255, 252, 248, 0.52);
}
.context-update > span {
  color: #a38d80;
  font-size: 12px;
}
.context-update p {
  display: -webkit-box;
  overflow: hidden;
  margin: 6px 0 0;
  color: #65534a;
  font-size: 13px;
  line-height: 1.55;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.context-update time {
  display: block;
  margin-top: 6px;
  color: #ac998d;
  font-size: 12px;
}
.feedback-main {
  min-width: 0;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.84);
  border-radius: 24px;
  background: var(--surface-soft);
  box-shadow: 0 16px 44px rgba(94, 62, 44, 0.1);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
}
.feedback-main :deep(.discussion-danmaku) {
  margin: 16px 26px 0;
}
.feedback-toolbar {
  position: sticky;
  top: 66px;
  z-index: 3;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 16px 26px;
  background: rgba(255, 250, 246, 0.9);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
}
.feedback-list {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  align-items: start;
  padding: 0 20px;
  margin-top: 10px;
}
.feedback-list.reviews {
  grid-template-columns: minmax(0, 1fr);
}
.feedback-skeletons {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  padding: 0 20px;
}
.skeleton-card {
  display: grid;
  min-height: 158px;
  grid-template-columns: 40px minmax(0, 1fr);
  gap: 13px;
  padding: 18px;
  border: 1px solid #efe5dc;
  border-radius: 16px;
  background: rgba(255, 254, 251, 0.66);
}
.skeleton-card > div {
  display: grid;
  align-content: start;
  gap: 11px;
}
.skeleton-avatar,
.skeleton-line {
  display: block;
  background: linear-gradient(90deg, #f1e7df, #f8f1eb, #f1e7df);
  background-size: 220% 100%;
  animation: skeleton-wave 1.6s ease-in-out infinite;
}
.skeleton-avatar {
  width: 40px;
  height: 40px;
  border-radius: 13px;
}
.skeleton-line {
  height: 10px;
  border-radius: 5px;
}
.skeleton-line.short {
  width: 36%;
}
.skeleton-line.medium {
  width: 72%;
}
.feedback-state {
  display: flex;
  min-height: 330px;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 9px;
  margin: 0 20px 16px;
  border: 1px dashed #eadbd0;
  border-radius: 18px;
  color: #9d877a;
  font-size: 13px;
  text-align: center;
}
.feedback-state strong {
  color: #705b50;
  font-size: 14px;
  font-weight: 600;
}
.state-icon {
  display: grid;
  width: 54px;
  height: 54px;
  margin-bottom: 3px;
  border: 1px solid rgba(231, 207, 192, 0.88);
  border-radius: 18px;
  color: #b17a63;
  background: rgba(255, 249, 244, 0.82);
  box-shadow: 0 8px 20px rgba(124, 81, 59, 0.08);
  place-items: center;
}
.feedback-state.error {
  color: #ad6654;
}
.feedback-state.missing {
  min-height: 60dvh;
  margin: 34px;
}
.feedback-state button,
.footer-retry {
  min-height: 32px;
  margin-top: 2px;
  padding: 0 13px;
  border: 1px solid #e7c6b5;
  border-radius: 9px;
  color: #a75b40;
  background: #fffaf6;
  font: inherit;
  font-size: 12px;
}
.feedback-footer {
  display: flex;
  min-height: 58px;
  align-items: center;
  justify-content: center;
  color: #aa978b;
  font-size: 12px;
}
.feedback-loading {
  display: inline-flex;
  align-items: center;
  gap: 7px;
}
@keyframes skeleton-wave {
  to {
    background-position: -220% 0;
  }
}
@media (max-width: 1080px) {
  .feedback-layout {
    grid-template-columns: 240px minmax(0, 1fr);
    gap: 20px;
  }
  .feedback-list,
  .feedback-skeletons {
    grid-template-columns: minmax(0, 1fr);
  }
}
@media (max-width: 900px) {
  .feedback-page {
    background: #fff;
  }
  .feedback-topbar {
    min-height: 59px;
    background: #fff;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }
  .topbar-inner {
    width: 100%;
    min-height: 59px;
    gap: 10px;
    padding: 9px 14px;
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
  .topbar-path span:not(.topbar-book),
  .topbar-path svg {
    display: none;
  }
  .topbar-book {
    max-width: 52vw;
    color: #6b5d54;
    font-size: 14px;
    font-weight: 600;
  }
  .topbar-path strong {
    display: none;
  }
  .detail-link {
    width: 40px;
    height: 40px;
    min-height: 40px;
    justify-content: center;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: #f5f3f0;
  }
  .detail-link span,
  .detail-link svg:last-child {
    display: none;
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
    overflow: visible;
    border: 0;
    border-radius: 0;
    background: #fff;
    box-shadow: none;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }
  .feedback-main :deep(.discussion-danmaku) {
    display: none;
  }
  .feedback-toolbar {
    top: 59px;
    padding: 13px 0 11px;
    background: #fff;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }
  .feedback-list,
  .feedback-skeletons {
    grid-template-columns: minmax(0, 1fr);
    gap: 10px;
    padding: 0;
  }
  .feedback-list {
    gap: 0;
    border-top: 1px solid #eee5db;
  }
  .feedback-list :deep(.book-comment + .book-comment) {
    border-top: 1px solid #f0e8df;
  }
  .feedback-list :deep(.book-comment),
  .skeleton-card {
    min-height: 0;
  }
  .skeleton-card {
    padding: 15px;
  }
  .feedback-state {
    min-height: 260px;
    margin: 12px 0 0;
  }
  .feedback-footer {
    min-height: 52px;
  }
}
@media (prefers-reduced-motion: reduce) {
  .back-button,
  .detail-link,
  .context-detail,
  .skeleton-avatar,
  .skeleton-line {
    transition: none;
    animation: none;
  }
}
</style>
