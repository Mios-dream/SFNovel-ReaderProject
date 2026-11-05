<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { computed, inject, onMounted, ref } from "vue";
import {
  ArrowRight,
  BookOpen,
  Eye,
  Flame,
  LoaderCircle,
  RotateCw,
  Search,
  Star,
} from "@lucide/vue";
import NovelCard from "../components/NovelCard.vue";
import PageFrame from "../components/PageFrame.vue";
import { nativeErrorMessage } from "../composables/deskShared";
import { deskInjectionKey } from "../deskContext";
import type { DiscoverFeed, Novel } from "../types";

function requireDesk() {
  const desk = inject(deskInjectionKey);
  if (!desk) throw new Error("Desk context is unavailable");
  return desk;
}

const desk = requireDesk();
const home = ref<DiscoverFeed>();
const homeLoading = ref(false);
const homeError = ref("");
const activeCategory = ref<number>();
let homeRequest = 0;

const featured = computed(() => home.value?.featured);
const categories = computed(() => home.value?.categories || []);
const ranking = computed(() => home.value?.ranking || []);
const recent = computed(() => home.value?.recent || []);

async function loadHome(typeId?: number) {
  const request = ++homeRequest;
  activeCategory.value = typeId;
  homeLoading.value = true;
  homeError.value = "";
  try {
    const feed = await invoke<DiscoverFeed>("get_discover_feed", {
      typeId: typeId ?? null,
    });
    if (request === homeRequest) home.value = feed;
  } catch (error) {
    if (request === homeRequest)
      homeError.value = nativeErrorMessage(error, "首页内容加载失败");
  } finally {
    if (request === homeRequest) homeLoading.value = false;
  }
}

function selectCategory(typeId?: number) {
  if (activeCategory.value === typeId) return;
  void loadHome(typeId);
}

function openNovel(novel: Novel) {
  desk.openRemoteBookDetail(novel);
}

function formatMetric(value?: number) {
  if (typeof value !== "number" || !Number.isFinite(value)) return "";
  if (value >= 100_000_000) return `${trimMetric(value / 100_000_000)}亿`;
  if (value >= 10_000) return `${trimMetric(value / 10_000)}万`;
  return Math.round(value).toLocaleString("zh-CN");
}

function trimMetric(value: number) {
  return value >= 100 ? value.toFixed(0) : value.toFixed(1).replace(/\.0$/, "");
}

onMounted(() => void loadHome());
</script>

<template>
  <PageFrame>
    <main class="discover-page">
      <div class="home-shell">
        <template v-if="!desk.searched.value">
          <section v-if="homeLoading && !home" class="hero home-loading">
            <LoaderCircle class="spin" :size="24" />
            <span>正在载入推荐</span>
          </section>

          <section v-else-if="featured" class="hero">
            <div
              v-if="featured.novelCover"
              class="hero-backdrop"
              :style="{ backgroundImage: `url('${featured.novelCover}')` }"
              aria-hidden="true"
            />
            <div class="hero-inner">
              <div class="hero-copy">
                <span class="hero-kicker"
                  ><Flame :size="14" fill="currentColor" />本周热门</span
                >
                <h1>{{ featured.novelName }}</h1>
                <p class="hero-author">
                  {{ featured.authorName || "未知作者" }}
                </p>
                <p class="hero-summary">
                  {{ featured.description || "暂无简介" }}
                </p>
                <div class="hero-meta">
                  <span v-if="featured.score != null"
                    ><Star :size="14" fill="currentColor" />{{
                      featured.score.toFixed(1)
                    }}</span
                  >
                  <span v-if="featured.viewCount != null"
                    ><Eye :size="14" />{{
                      formatMetric(featured.viewCount)
                    }}</span
                  >
                  <span>{{ featured.isFinish ? "已完结" : "连载中" }}</span>
                </div>
                <button class="hero-cta" @click="openNovel(featured)">
                  查看作品<ArrowRight :size="18" />
                </button>
              </div>
              <button
                class="hero-cover"
                :aria-label="`查看 ${featured.novelName}`"
                @click="openNovel(featured)"
              >
                <img
                  v-if="featured.novelCover"
                  :src="featured.novelCover"
                  :alt="`${featured.novelName} 封面`"
                />
                <BookOpen v-else :size="48" />
              </button>
            </div>
          </section>

          <section v-else-if="homeError" class="empty-state home-error">
            <BookOpen :size="28" />
            <p>{{ homeError }}</p>
            <button @click="loadHome(activeCategory)">
              <RotateCw :size="15" />重新加载
            </button>
          </section>

          <form class="search-box glass" @submit.prevent="desk.search">
            <Search :size="21" />
            <input
              :value="desk.query.value"
              autocomplete="off"
              placeholder="搜索书名、作者或关键词"
              aria-label="搜索作品"
              @input="desk.query.value = ($event.target as HTMLInputElement).value"
            />
            <button
              class="primary-button"
              type="submit"
              :disabled="desk.loading.value"
            >
              <LoaderCircle
                v-if="desk.loading.value"
                class="spin"
                :size="17"
              /><span>{{ desk.loading.value ? "搜索中" : "搜索" }}</span>
            </button>
          </form>

          <template v-if="home">
            <section class="category-strip" aria-label="作品分类">
              <div class="category-list">
                <button
                  :class="{ active: activeCategory == null }"
                  :disabled="homeLoading"
                  @click="selectCategory()"
                >
                  全部
                </button>
                <button
                  v-for="category in categories"
                  :key="category.typeId"
                  :class="{ active: activeCategory === category.typeId }"
                  :disabled="homeLoading"
                  @click="selectCategory(category.typeId)"
                >
                  {{ category.typeName }}
                </button>
              </div>
              <LoaderCircle
                v-if="homeLoading"
                class="spin category-loading"
                :size="17"
                aria-label="正在刷新"
              />
            </section>

            <section v-if="ranking.length" class="feed-section">
              <header class="feed-heading">
                <h2>本周热门</h2>
              </header>
              <div class="ranking-grid">
                <button
                  v-for="(novel, index) in ranking"
                  :key="novel.novelId"
                  class="rank-card"
                  @click="openNovel(novel)"
                >
                  <span class="rank-cover">
                    <img
                      v-if="novel.novelCover"
                      :src="novel.novelCover"
                      :alt="`${novel.novelName} 封面`"
                    />
                    <BookOpen v-else :size="34" />
                    <span class="rank-index" :class="{ leading: index < 3 }">{{
                      String(index + 2).padStart(2, "0")
                    }}</span>
                  </span>
                  <strong class="rank-title">{{ novel.novelName }}</strong>
                  <span class="rank-author">{{ novel.authorName }}</span>
                  <span class="rank-meta">
                    <span v-if="novel.score != null"
                      ><Star :size="12" fill="currentColor" />{{
                        novel.score.toFixed(1)
                      }}</span
                    >
                    <span v-if="novel.viewCount != null">{{
                      formatMetric(novel.viewCount)
                    }}</span>
                  </span>
                </button>
              </div>
            </section>

            <section v-if="recent.length" class="feed-section recent-section">
              <header class="feed-heading">
                <h2>最近更新</h2>
              </header>
              <div class="recent-grid">
                <button
                  v-for="novel in recent"
                  :key="novel.novelId"
                  class="recent-card glass"
                  @click="openNovel(novel)"
                >
                  <span class="recent-cover">
                    <img
                      v-if="novel.novelCover"
                      :src="novel.novelCover"
                      :alt="`${novel.novelName} 封面`"
                    />
                    <BookOpen v-else :size="26" />
                  </span>
                  <span class="recent-copy">
                    <strong>{{ novel.novelName }}</strong>
                    <span class="recent-author">{{ novel.authorName }}</span>
                    <span class="recent-description">{{
                      novel.description || "暂无简介"
                    }}</span>
                    <span class="recent-footer">
                      <span v-if="novel.latestChapterTitle"
                        >更新至 {{ novel.latestChapterTitle }}</span
                      >
                      <time v-else>{{ desk.formatDate(novel.lastUpdateTime) }}</time>
                    </span>
                  </span>
                </button>
              </div>
            </section>
          </template>
        </template>

        <template v-else>
          <form class="search-box glass search-box-active" @submit.prevent="desk.search">
            <Search :size="21" />
            <input
              :value="desk.query.value"
              autocomplete="off"
              placeholder="搜索书名、作者或关键词"
              aria-label="搜索作品"
              @input="desk.query.value = ($event.target as HTMLInputElement).value"
            />
            <button
              class="primary-button"
              type="submit"
              :disabled="desk.loading.value"
            >
              <LoaderCircle
                v-if="desk.loading.value"
                class="spin"
                :size="17"
              /><span>{{ desk.loading.value ? "搜索中" : "搜索" }}</span>
            </button>
          </form>

          <section class="results-section">
            <header class="results-heading">
              <h2>
                {{
                  desk.loading.value
                    ? "正在搜索"
                    : `找到 ${desk.results.value.length} 个作品`
                }}
              </h2>
              <button class="clear-search" @click="desk.clearSearch">
                返回首页
              </button>
            </header>

            <div v-if="desk.loading.value" class="empty-state">
              <LoaderCircle class="spin" :size="26" />
              <p>正在查找作品</p>
            </div>
            <div v-else-if="!desk.results.value.length" class="empty-state">
              <BookOpen :size="28" />
              <p>没有找到匹配的作品</p>
            </div>
            <div v-else class="novel-grid">
              <NovelCard
                v-for="novel in desk.results.value"
                :key="`${novel.novelId}-${novel.bookshelfType || 'novel'}`"
                :novel="novel"
                :format-date="desk.formatDate"
                @select="desk.openChapterPicker"
                @detail="desk.openRemoteBookDetail"
              />
            </div>
          </section>
        </template>
      </div>
    </main>
  </PageFrame>
</template>

<style scoped>
.discover-page {
  min-width: 0;
  height: 100%;
  padding: 0 42px 34px 18px;
  overflow-y: auto;
  scrollbar-gutter: stable;
}
.home-shell {
  display: flex;
  width: min(1180px, 100%);
  flex-direction: column;
  margin: 0 auto;
}
.hero {
  position: relative;
  min-height: 318px;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.82);
  border-radius: 24px;
  background: linear-gradient(115deg, #fff9f3 8%, #ffeadf 58%, #f8cbb2);
  box-shadow: 0 16px 40px rgba(137, 76, 55, 0.13);
}
.hero::after {
  position: absolute;
  inset: 0;
  background: linear-gradient(
    90deg,
    rgba(255, 249, 243, 0.98) 0%,
    rgba(255, 245, 237, 0.92) 47%,
    rgba(255, 235, 223, 0.18) 100%
  );
  content: "";
}
.hero-backdrop {
  position: absolute;
  inset: -34px -18px -34px 37%;
  background-position: center 32%;
  background-size: cover;
  filter: blur(20px) saturate(1.08);
  opacity: 0.52;
  transform: scale(1.08);
}
.hero-inner {
  position: relative;
  z-index: 1;
  display: flex;
  min-height: 318px;
  align-items: center;
  justify-content: space-between;
  gap: 44px;
  padding: 34px 42px;
}
.hero-copy {
  display: flex;
  max-width: 690px;
  min-width: 0;
  flex-direction: column;
  align-items: flex-start;
}
.hero-kicker {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 6px 10px;
  border: 1px solid #f0bea4;
  border-radius: 999px;
  color: #a95436;
  background: rgba(255, 238, 227, 0.88);
  font-size: 12px;
  font-weight: 600;
}
.hero h1 {
  display: -webkit-box;
  margin: 16px 0 7px;
  overflow: hidden;
  color: #523832;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: clamp(31px, 3.4vw, 45px);
  line-height: 1.2;
  text-wrap: balance;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.hero-author {
  margin: 0;
  color: #9b6f61;
  font-size: 14px;
}
.hero-summary {
  display: -webkit-box;
  max-width: 660px;
  margin: 13px 0 0;
  overflow: hidden;
  color: #80655c;
  font-size: 14px;
  line-height: 1.75;
  white-space: pre-line;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
}
.hero-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 14px;
}
.hero-meta span {
  display: inline-flex;
  min-height: 27px;
  align-items: center;
  gap: 4px;
  padding: 0 9px;
  border: 1px solid rgba(227, 164, 132, 0.42);
  border-radius: 8px;
  color: #8b5d4d;
  background: rgba(255, 252, 248, 0.62);
  font-size: 12px;
}
.hero-meta span:first-child {
  color: #b46535;
}
.hero-cta {
  display: inline-flex;
  min-height: 42px;
  align-items: center;
  gap: 6px;
  margin-top: 19px;
  padding: 0 16px;
  border: 0;
  border-radius: 11px;
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 8px 16px rgba(185, 98, 59, 0.24);
  font-size: 14px;
  font-weight: 600;
  transition:
    background-color 160ms ease,
    transform 160ms ease,
    box-shadow 160ms ease;
}
.hero-cta:hover {
  background: var(--theme-color-dark);
  box-shadow: 0 10px 20px rgba(185, 98, 59, 0.3);
  transform: translateY(-1px);
}
.hero-cover {
  display: grid;
  width: 188px;
  height: 260px;
  flex: 0 0 188px;
  overflow: hidden;
  padding: 0;
  border: 5px solid rgba(255, 250, 245, 0.9);
  border-radius: 15px;
  color: #a56b54;
  background: #f2d4c3;
  box-shadow: 0 18px 34px rgba(92, 48, 34, 0.24);
  place-items: center;
  transform: rotate(2deg);
  transition:
    transform 200ms ease-out,
    box-shadow 200ms ease-out;
}
.hero-cover:hover {
  box-shadow: 0 23px 42px rgba(92, 48, 34, 0.3);
  transform: rotate(0deg) translateY(-3px);
}
.hero-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.home-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 9px;
  color: #a77a6b;
  font-size: 14px;
}
.search-box {
  display: flex;
  align-items: center;
  gap: 11px;
  margin-top: 16px;
  padding: 8px 8px 8px 17px;
  border-radius: 15px;
  color: #b17f70;
  background: rgba(255, 255, 255, 0.72);
}
.search-box-active {
  margin-top: 0;
}
.search-box input {
  flex: 1;
  min-width: 0;
  border: 0;
  outline: 0;
  color: #5f423c;
  background: transparent;
  font-size: 14px;
}
.search-box input::placeholder {
  color: #bd998d;
}
.primary-button {
  display: inline-flex;
  min-height: 39px;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 0 17px;
  border: 0;
  border-radius: 10px;
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 7px 12px rgba(226, 148, 100, 0.25);
  font-size: 14px;
  font-weight: 600;
  transition:
    background-color 160ms ease,
    transform 160ms ease;
}
.primary-button:hover:not(:disabled) {
  background: var(--theme-color-dark);
  transform: translateY(-1px);
}
.primary-button:disabled {
  cursor: wait;
  opacity: 0.72;
}
.category-strip {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 10px;
  margin-top: 20px;
}
.category-list {
  display: flex;
  min-width: 0;
  flex: 1 1 auto;
  gap: 7px;
  overflow-x: auto;
  scrollbar-width: none;
}
.category-list button {
  min-height: 32px;
  flex: 0 0 auto;
  padding: 0 13px;
  border: 1px solid rgba(230, 190, 170, 0.72);
  border-radius: 999px;
  color: #8d6d62;
  background: rgba(255, 253, 249, 0.56);
  font-size: 12px;
  transition:
    border-color 150ms ease,
    background-color 150ms ease,
    color 150ms ease;
}
.category-list button:hover:not(:disabled) {
  border-color: #e9ae8d;
  color: #b75b3e;
  background: #fff4ec;
}
.category-list button.active {
  border-color: var(--theme-color);
  color: #fff;
  background: var(--theme-color);
}
.category-list button:disabled {
  cursor: default;
}
.category-loading {
  flex: 0 0 auto;
  color: #b9826d;
}
.feed-section {
  margin-top: 31px;
}
.feed-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 14px;
}
.feed-heading h2,
.results-heading h2 {
  margin: 0;
  color: #5b4039;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 22px;
}
.ranking-grid {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 15px;
}
.rank-card {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 6px;
  padding: 0;
  border: 0;
  color: inherit;
  background: transparent;
  text-align: left;
}
.rank-cover {
  position: relative;
  display: grid;
  width: 100%;
  aspect-ratio: 3 / 4.15;
  overflow: hidden;
  margin-bottom: 3px;
  border-radius: 14px;
  color: #a56b54;
  background: #f2d7c8;
  box-shadow: 0 10px 22px rgba(91, 55, 41, 0.13);
  place-items: center;
  transition:
    box-shadow 180ms ease-out,
    transform 180ms ease-out;
}
.rank-cover::after {
  position: absolute;
  inset: auto 0 0;
  height: 34%;
  background: linear-gradient(transparent, rgba(39, 25, 20, 0.22));
  content: "";
  pointer-events: none;
}
.rank-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 220ms ease-out;
}
.rank-card:hover .rank-cover {
  box-shadow: 0 15px 28px rgba(91, 55, 41, 0.21);
  transform: translateY(-3px);
}
.rank-card:hover .rank-cover img {
  transform: scale(1.025);
}
.rank-card:focus-visible {
  outline: 0;
}
.rank-card:focus-visible .rank-cover {
  outline: 2px solid var(--theme-color);
  outline-offset: 3px;
}
.rank-index {
  position: absolute;
  top: 9px;
  left: 9px;
  z-index: 1;
  display: grid;
  min-width: 28px;
  height: 25px;
  padding: 0 6px;
  border-radius: 7px;
  color: #775f54;
  background: rgba(255, 251, 246, 0.88);
  font: 12px monospace;
  place-items: center;
  backdrop-filter: blur(7px);
}
.rank-index.leading {
  color: #fff;
  background: var(--theme-color);
}
.rank-title {
  display: -webkit-box;
  min-height: 40px;
  overflow: hidden;
  color: #5c4039;
  font-size: 14px;
  font-weight: 600;
  line-height: 1.45;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.rank-author {
  overflow: hidden;
  color: #9b7c70;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rank-meta {
  display: flex;
  min-height: 18px;
  align-items: center;
  gap: 8px;
  color: #b07c68;
  font-size: 12px;
}
.rank-meta span {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}
.recent-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}
.recent-card {
  display: grid;
  min-width: 0;
  grid-template-columns: 64px minmax(0, 1fr);
  gap: 12px;
  padding: 10px;
  border-radius: 15px;
  color: inherit;
  background: rgba(255, 253, 249, 0.6);
  text-align: left;
  transition:
    background-color 160ms ease,
    box-shadow 160ms ease,
    transform 160ms ease;
}
.recent-card:hover {
  background: rgba(255, 252, 248, 0.86);
  box-shadow: 0 15px 28px rgba(137, 76, 55, 0.14);
  transform: translateY(-2px);
}
.recent-card:focus-visible {
  outline: 2px solid var(--theme-color);
  outline-offset: 2px;
}
.recent-cover {
  display: grid;
  width: 64px;
  height: 91px;
  overflow: hidden;
  border-radius: 9px;
  color: #a56b54;
  background: #f2d7c8;
  place-items: center;
}
.recent-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.recent-copy {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 3px;
  padding: 1px 2px 0 0;
}
.recent-copy strong {
  display: -webkit-box;
  overflow: hidden;
  color: #5d423b;
  font-size: 14px;
  font-weight: 600;
  line-height: 1.45;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.recent-author {
  overflow: hidden;
  color: #a17f73;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.recent-description {
  display: -webkit-box;
  overflow: hidden;
  color: #887168;
  font-size: 12px;
  line-height: 1.5;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.recent-footer {
  overflow: hidden;
  margin-top: auto;
  color: #b28c7d;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.results-section {
  margin-top: 24px;
}
.results-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 14px;
}
.clear-search {
  padding: 7px 11px;
  border: 1px solid #e7c8b8;
  border-radius: 9px;
  color: #9a624d;
  background: rgba(255, 252, 248, 0.62);
  font-size: 12px;
  transition:
    border-color 150ms ease,
    background-color 150ms ease,
    color 150ms ease;
}
.clear-search:hover {
  border-color: #df9d7e;
  color: #b65b3d;
  background: #fff3ea;
}
.novel-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}
.empty-state {
  display: grid;
  min-height: 190px;
  gap: 9px;
  border: 1px dashed #e6bca7;
  border-radius: 15px;
  color: #b28c81;
  background: rgba(255, 250, 247, 0.26);
  font-size: 14px;
  justify-items: center;
  place-content: center;
}
.empty-state p {
  margin: 0;
}
.empty-state button {
  display: inline-flex;
  min-height: 32px;
  align-items: center;
  gap: 5px;
  padding: 0 11px;
  border: 1px solid #e7c1ae;
  border-radius: 8px;
  color: #a75b40;
  background: rgba(255, 250, 246, 0.72);
  font-size: 12px;
}
.home-error {
  min-height: 318px;
  border-style: solid;
  border-color: rgba(255, 255, 255, 0.78);
  background: rgba(255, 249, 245, 0.62);
}

@media (max-width: 1050px) {
  .ranking-grid {
    grid-template-columns: repeat(5, minmax(108px, 1fr));
    overflow-x: auto;
    padding-bottom: 5px;
  }
  .recent-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 760px) {
  .discover-page {
    height: auto;
    padding: 10px 0 0;
    overflow: visible;
  }
  .hero {
    min-height: 258px;
    border-radius: 20px;
  }
  .hero::after {
    background: linear-gradient(
      90deg,
      rgba(255, 249, 243, 0.98) 0%,
      rgba(255, 242, 232, 0.84) 68%,
      rgba(255, 235, 223, 0.12) 100%
    );
  }
  .hero-backdrop {
    inset: -20px -12px -20px 24%;
    opacity: 0.46;
  }
  .hero-inner {
    min-height: 258px;
    align-items: flex-end;
    gap: 13px;
    padding: 24px 18px 25px;
  }
  .hero-copy {
    flex: 1 1 auto;
  }
  .hero-kicker {
    padding: 5px 9px;
    font-size: 12px;
  }
  .hero h1 {
    margin-top: 12px;
    font-size: clamp(25px, 7.5vw, 31px);
    -webkit-line-clamp: 2;
  }
  .hero-author {
    font-size: 12px;
  }
  .hero-summary {
    margin-top: 9px;
    font-size: 12px;
    line-height: 1.65;
    -webkit-line-clamp: 2;
  }
  .hero-meta {
    gap: 6px;
    margin-top: 10px;
  }
  .hero-meta span {
    min-height: 24px;
    padding: 0 7px;
    font-size: 12px;
  }
  .hero-meta span:nth-child(3) {
    display: none;
  }
  .hero-cta {
    min-height: 38px;
    margin-top: 13px;
    padding: 0 13px;
    font-size: 12px;
  }
  .hero-cover {
    width: 91px;
    height: 129px;
    flex-basis: 91px;
    border-width: 3px;
    border-radius: 10px;
  }
  .home-loading {
    min-height: 258px;
  }
  .search-box {
    margin-top: 10px;
    padding: 6px 6px 6px 13px;
    border-radius: 13px;
  }
  .search-box-active {
    margin-top: 0;
  }
  .search-box input {
    font-size: 13px;
  }
  .primary-button {
    min-height: 37px;
    padding: 0 14px;
  }
  .category-strip {
    margin-top: 14px;
  }
  .category-list button {
    min-height: 30px;
    padding: 0 11px;
  }
  .feed-section {
    margin-top: 25px;
  }
  .feed-heading h2,
  .results-heading h2 {
    font-size: 20px;
  }
  .ranking-grid {
    grid-template-columns: none;
    grid-auto-columns: 134px;
    grid-auto-flow: column;
    gap: 11px;
    overflow-x: auto;
    padding: 0 0 6px;
    scroll-snap-type: x proximity;
  }
  .rank-card {
    scroll-snap-align: start;
  }
  .rank-cover {
    border-radius: 12px;
  }
  .rank-title {
    min-height: 40px;
  }
  .recent-grid {
    grid-template-columns: 1fr;
    gap: 9px;
  }
  .recent-card {
    grid-template-columns: 62px minmax(0, 1fr);
    padding: 9px;
    border-radius: 14px;
  }
  .recent-cover {
    width: 62px;
    height: 88px;
  }
  .results-section {
    margin-top: 18px;
  }
  .novel-grid {
    grid-template-columns: 1fr;
  }
  .home-error {
    min-height: 258px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .hero-cta,
  .hero-cover,
  .primary-button,
  .category-list button,
  .rank-cover,
  .rank-cover img,
  .recent-card,
  .clear-search {
    transition: none;
  }
}
</style>
