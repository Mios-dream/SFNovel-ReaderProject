<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  BookOpen,
  ChevronLeft,
  ChevronRight,
  Eye,
  LoaderCircle,
  RotateCw,
  Search,
  Star,
} from "@lucide/vue";
import NovelCard from "../components/NovelCard.vue";
import PageFrame from "../components/PageFrame.vue";
import { nativeErrorMessage } from "../utils/native";
import type { DiscoverFeed, Novel } from "../types";
import { extractDominantColor } from "../utils/dominantColor";
import { trimMetric } from "../utils/format";

import { usePicker, useRemote, useSearch } from "../desk";
import { formatMonthDay } from "../utils/format";

const picker = usePicker();
const remote = useRemote();
const search = useSearch();

/** 取色不可用时的主题色，与全局主色一致。 */
const HERO_TINT_FALLBACK = "rgb(226 148 100)";

const home = ref<DiscoverFeed>();
const homeLoading = ref(false);
const homeError = ref("");
const activeCategory = ref<number>();
/** 轨道虚拟位置：0 为"末尾克隆"，1..total 为真实页，total+1 为"开头克隆"。 */
const virtualIndex = ref(1);
const sliding = ref(false);
const silentMove = ref(false);
const carouselPaused = ref(false);
const heroTint = ref(HERO_TINT_FALLBACK);
let homeRequest = 0;
let carouselTimer: number | undefined;
let slideTimer: number | undefined;
let tintRequest = 0;
const coverTints = new Map<string, string>();

const categories = computed(() => home.value?.categories || []);
const ranking = computed(() => home.value?.ranking || []);
const recent = computed(() => home.value?.recent || []);
const heroSlides = computed(() => {
  const slides: Novel[] = [];
  if (home.value?.featured) slides.push(home.value.featured);
  for (const novel of ranking.value) {
    if (!slides.some((slide) => slide.novelId === novel.novelId))
      slides.push(novel);
  }
  return slides;
});
const activeSlideIndex = computed(() => {
  const total = heroSlides.value.length;
  if (!total) return 0;
  return (((virtualIndex.value - 1) % total) + total) % total;
});
const activeSlide = computed(() => heroSlides.value[activeSlideIndex.value]);
/** 渲染轨道：首尾各克隆一张，实现无缝循环。 */
const trackSlides = computed(() => {
  const slides = heroSlides.value;
  if (slides.length < 2)
    return slides.map((novel) => ({ novel, key: String(novel.novelId) }));
  const first = slides[0]!;
  const last = slides[slides.length - 1]!;
  return [
    { novel: last, key: `head-clone-${last.novelId}` },
    ...slides.map((novel) => ({ novel, key: String(novel.novelId) })),
    { novel: first, key: `tail-clone-${first.novelId}` },
  ];
});

async function loadHome(typeId?: number) {
  const request = ++homeRequest;
  activeCategory.value = typeId;
  homeLoading.value = true;
  homeError.value = "";
  try {
    const feed = await invoke<DiscoverFeed>("get_discover_feed", {
      typeId: typeId ?? null,
    });
    if (request === homeRequest) {
      home.value = feed;
      resetCarouselState();
    }
  } catch (error) {
    if (request === homeRequest)
      homeError.value = nativeErrorMessage(error, "首页内容加载失败");
  } finally {
    if (request === homeRequest) homeLoading.value = false;
  }
}

function resetCarouselState() {
  if (slideTimer != null) {
    window.clearTimeout(slideTimer);
    slideTimer = undefined;
  }
  sliding.value = false;
  silentMove.value = false;
  virtualIndex.value = 1;
}

function selectCategory(typeId?: number) {
  if (activeCategory.value === typeId) return;
  void loadHome(typeId);
}

function openNovel(novel: Novel) {
  remote.openRemoteBookDetail(novel);
}

const trackTransitionMs = 320;

/** 带过渡的轨道移动；结束后若停在克隆页，则无声跳回真实页。 */
function animateTo(position: number) {
  if (sliding.value) return;
  sliding.value = true;
  virtualIndex.value = position;
  if (slideTimer != null) window.clearTimeout(slideTimer);
  slideTimer = window.setTimeout(() => {
    slideTimer = undefined;
    sliding.value = false;
    settleTrack();
  }, trackTransitionMs + 40);
}

/** 落在克隆页时关掉过渡瞬移回真实页，实现无缝循环。 */
function settleTrack() {
  const total = heroSlides.value.length;
  if (total < 2) return;
  const last = total + 1;
  if (virtualIndex.value === last) jumpTo(1);
  else if (virtualIndex.value === 0) jumpTo(total);
}

function jumpTo(position: number) {
  silentMove.value = true;
  virtualIndex.value = position;
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      silentMove.value = false;
    }),
  );
}

function showSlide(index: number) {
  const total = heroSlides.value.length;
  if (!total) return;
  animateTo(index + 1);
}

function previousSlide() {
  if (heroSlides.value.length < 2) return;
  animateTo(virtualIndex.value - 1);
}

function nextSlide() {
  if (heroSlides.value.length < 2) return;
  animateTo(virtualIndex.value + 1);
}

/** 轮播手势滑动：pointer 事件统一处理触摸与鼠标拖拽。 */
const dragDX = ref(0);
const dragLocked = ref(false);
let dragPointerId: number | null = null;
let dragStartX = 0;
let dragStartY = 0;
let dragStartTime = 0;
let dragMoved = false;

const prefersReducedMotion = window.matchMedia(
  "(prefers-reduced-motion: reduce)",
);

const trackStyle = computed(() => {
  if (heroSlides.value.length < 2) return undefined;
  const noTransition =
    dragLocked.value || silentMove.value || prefersReducedMotion.matches;
  return {
    transform: `translate3d(calc(${virtualIndex.value * -100}% + ${dragDX.value}px), 0, 0)`,
    transition:
      noTransition
        ? "none"
        : `transform ${trackTransitionMs}ms cubic-bezier(0.22, 0.61, 0.36, 1)`,
  };
});

function onHeroPointerDown(event: PointerEvent) {
  if (heroSlides.value.length < 2 || sliding.value) return;
  if (event.pointerType === "mouse" && event.button !== 0) return;
  const target = event.target as HTMLElement | null;
  if (target?.closest("button")) return; // 箭头/圆点按钮不参与滑动
  dragPointerId = event.pointerId;
  dragStartX = event.clientX;
  dragStartY = event.clientY;
  dragStartTime = performance.now();
  dragMoved = false;
}

function onHeroPointerMove(event: PointerEvent) {
  if (event.pointerId !== dragPointerId) return;
  const dx = event.clientX - dragStartX;
  const dy = event.clientY - dragStartY;
  if (!dragLocked.value) {
    if (Math.abs(dx) < 8 && Math.abs(dy) < 8) return;
    if (Math.abs(dx) <= Math.abs(dy)) {
      // 垂直方向意图（页面滚动），放弃手势
      dragPointerId = null;
      return;
    }
    dragLocked.value = true;
    (event.currentTarget as HTMLElement)
      .setPointerCapture?.(event.pointerId);
  }
  dragDX.value = dx;
  if (Math.abs(dx) > 8) dragMoved = true;
}

function onHeroPointerUp(event: PointerEvent) {
  if (event.pointerId !== dragPointerId) return;
  const wasLocked = dragLocked.value;
  const dx = dragDX.value;
  const elapsed = performance.now() - dragStartTime;
  const width = (event.currentTarget as HTMLElement).clientWidth || 1;
  const threshold = Math.max(48, width * 0.12);
  const flick = Math.abs(dx) > 30 && elapsed < 260;
  resetDrag();
  if (Math.abs(dx) >= threshold || flick) {
    if (dx < 0) nextSlide();
    else previousSlide();
  } else if (wasLocked) {
    // 未过阈值：弹回当前页
    animateTo(virtualIndex.value);
  }
}

function onHeroPointerCancel(event: PointerEvent) {
  if (event.pointerId !== dragPointerId) return;
  const wasLocked = dragLocked.value;
  resetDrag();
  if (wasLocked) animateTo(virtualIndex.value);
}

function resetDrag() {
  dragPointerId = null;
  dragLocked.value = false;
  dragDX.value = 0;
}

function onHeroClickCapture(event: MouseEvent) {
  if (dragMoved) {
    // 拖拽结束后的 click 不应触发打开作品
    event.stopPropagation();
    event.preventDefault();
    dragMoved = false;
  }
}

function conciseDescription(value?: string) {
  return value?.replace(/\s+/g, " ").trim() || "暂无简介";
}

async function updateHeroTint(cover?: string) {
  const request = ++tintRequest;
  if (!cover) {
    heroTint.value = HERO_TINT_FALLBACK;
    return;
  }
  const cached = coverTints.get(cover);
  if (cached) {
    heroTint.value = cached;
    return;
  }

  const color = await extractDominantColor(cover);
  if (request !== tintRequest) return;
  if (!color) {
    heroTint.value = HERO_TINT_FALLBACK;
    return;
  }
  coverTints.set(cover, color);
  heroTint.value = color;
}

function restartCarousel() {
  if (carouselTimer != null) window.clearInterval(carouselTimer);
  carouselTimer = undefined;
  if (
    heroSlides.value.length < 2 ||
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  )
    return;
  carouselTimer = window.setInterval(() => {
    if (!carouselPaused.value && !dragLocked.value && !sliding.value)
      nextSlide();
  }, 6500);
}

function formatMetric(value?: number) {
  if (typeof value !== "number" || !Number.isFinite(value)) return "";
  if (value >= 100_000_000) return `${trimMetric(value / 100_000_000)}亿`;
  if (value >= 10_000) return `${trimMetric(value / 10_000)}万`;
  return Math.round(value).toLocaleString("zh-CN");
}

watch(heroSlides, restartCarousel, { flush: "post" });
watch(carouselPaused, restartCarousel);
watch(activeSlideIndex, restartCarousel);
watch(() => activeSlide.value?.novelCover, updateHeroTint, { immediate: true });

onMounted(() => void loadHome());
onBeforeUnmount(() => {
  if (carouselTimer != null) window.clearInterval(carouselTimer);
  if (slideTimer != null) window.clearTimeout(slideTimer);
  tintRequest += 1;
});
</script>

<template>
  <PageFrame>
    <main class="discover-page">
      <div class="home-shell">
        <template v-if="!search.searched.value">
          <section v-if="homeLoading && !home" class="hero home-loading">
            <LoaderCircle class="spin" :size="24" />
            <span>正在载入推荐</span>
          </section>

          <section
            v-else-if="activeSlide"
            class="hero"
            :class="{ dragging: dragLocked }"
            :style="{ '--hero-tint': heroTint }"
            role="region"
            aria-roledescription="轮播图"
            aria-label="本周热门"
            @mouseenter="carouselPaused = true"
            @mouseleave="carouselPaused = false"
            @focusin="carouselPaused = true"
            @focusout="carouselPaused = false"
            @pointerdown="onHeroPointerDown"
            @pointermove="onHeroPointerMove"
            @pointerup="onHeroPointerUp"
            @pointercancel="onHeroPointerCancel"
            @click.capture="onHeroClickCapture"
          >
            <div class="hero-track" :style="trackStyle">
              <div
                v-for="(entry, index) in trackSlides"
                :key="entry.key"
                class="hero-slide"
                :class="{ 'is-active': index === virtualIndex }"
                :style="{ left: `${index * 100}%` }"
                :aria-hidden="index !== virtualIndex"
              >
                <img
                  v-if="entry.novel.novelCover"
                  class="hero-art"
                  :src="entry.novel.novelCover"
                  :alt="`${entry.novel.novelName} 封面`"
                  draggable="false"
                />
                <BookOpen v-else class="hero-art-placeholder" :size="40" />
                <div
                  class="hero-inner"
                  role="link"
                  :tabindex="index === virtualIndex ? 0 : -1"
                  :aria-label="`查看 ${entry.novel.novelName}`"
                  @click="openNovel(entry.novel)"
                  @keydown.enter="openNovel(entry.novel)"
                >
                  <div class="hero-copy">
                    <h1>{{ entry.novel.novelName }}</h1>
                    <p class="hero-author">
                      著 / {{ entry.novel.authorName || "未知作者" }}
                    </p>
                  </div>
                  <div class="hero-description">
                    <p>{{ conciseDescription(entry.novel.description) }}</p>
                    <div class="hero-meta">
                      <span v-if="entry.novel.score != null"
                        ><Star :size="13" fill="currentColor" />{{
                          entry.novel.score.toFixed(1)
                        }}</span
                      >
                      <span v-if="entry.novel.viewCount != null"
                        ><Eye :size="13" />{{
                          formatMetric(entry.novel.viewCount)
                        }}</span
                      >
                      <span>{{
                        entry.novel.isFinish ? "已完结" : "连载中"
                      }}</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
            <div v-if="heroSlides.length > 1" class="hero-controls">
              <button
                class="hero-arrow"
                type="button"
                aria-label="上一部热门作品"
                @click="previousSlide"
              >
                <ChevronLeft :size="16" />
              </button>
              <div class="hero-dots" role="tablist" aria-label="热门作品">
                <button
                  v-for="(novel, index) in heroSlides"
                  :key="novel.novelId"
                  type="button"
                  role="tab"
                  :class="{ active: index === activeSlideIndex }"
                  :aria-label="`查看第 ${index + 1} 部热门作品`"
                  :aria-selected="index === activeSlideIndex"
                  @click="showSlide(index)"
                />
              </div>
              <button
                class="hero-arrow"
                type="button"
                aria-label="下一部热门作品"
                @click="nextSlide"
              >
                <ChevronRight :size="16" />
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

          <form class="search-box glass" @submit.prevent="search.search">
            <Search :size="21" />
            <input
              :value="search.query.value"
              autocomplete="off"
              placeholder="搜索书名、作者或关键词"
              aria-label="搜索作品"
              @input="
                search.query.value = ($event.target as HTMLInputElement).value
              "
            />
            <button
              class="primary-button"
              type="submit"
              :disabled="search.loading.value"
            >
              <LoaderCircle
                v-if="search.loading.value"
                class="spin"
                :size="17"
              /><span>{{ search.loading.value ? "搜索中" : "搜索" }}</span>
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
                  <!-- <span class="rank-author">{{ novel.authorName }}</span> -->
                  <span v-if="novel.tags?.length" class="rank-tags">
                    <span v-for="tag in novel.tags.slice(0, 2)" :key="tag">{{
                      tag
                    }}</span>
                  </span>
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
                      <template v-if="novel.tags?.length">
                        <span
                          v-for="tag in novel.tags.slice(0, 2)"
                          :key="tag"
                          class="recent-tag"
                          >{{ tag }}</span
                        >
                      </template>
                      <template v-else>
                        <span v-if="novel.latestChapterTitle"
                          >更新至 {{ novel.latestChapterTitle }}</span
                        >
                        <time v-else>{{
                          formatMonthDay(novel.lastUpdateTime)
                        }}</time>
                      </template>
                    </span>
                  </span>
                </button>
              </div>
            </section>
          </template>
        </template>

        <template v-else>
          <form
            class="search-box glass search-box-active"
            @submit.prevent="search.search"
          >
            <Search :size="21" />
            <input
              :value="search.query.value"
              autocomplete="off"
              placeholder="搜索书名、作者或关键词"
              aria-label="搜索作品"
              @input="
                search.query.value = ($event.target as HTMLInputElement).value
              "
            />
            <button
              class="primary-button"
              type="submit"
              :disabled="search.loading.value"
            >
              <LoaderCircle
                v-if="search.loading.value"
                class="spin"
                :size="17"
              /><span>{{ search.loading.value ? "搜索中" : "搜索" }}</span>
            </button>
          </form>

          <section class="results-section">
            <header class="results-heading">
              <h2>
                {{
                  search.loading.value
                    ? "正在搜索"
                    : `找到 ${search.results.value.length} 个作品`
                }}
              </h2>
              <button class="clear-search" @click="search.clearSearch">
                返回首页
              </button>
            </header>

            <div v-if="search.loading.value" class="empty-state">
              <LoaderCircle class="spin" :size="26" />
              <p>正在查找作品</p>
            </div>
            <div v-else-if="!search.results.value.length" class="empty-state">
              <BookOpen :size="28" />
              <p>没有找到匹配的作品</p>
            </div>
            <div v-else class="novel-grid">
              <NovelCard
                v-for="novel in search.results.value"
                :key="`${novel.novelId}-${novel.bookshelfType || 'novel'}`"
                :novel="novel"
                :format-date="formatMonthDay"
                @select="picker.openChapterPicker"
                @detail="remote.openRemoteBookDetail"
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
  --hero-art-width: clamp(280px, 36%, 400px);
  min-height: 252px;
  isolation: isolate;
  overflow: hidden;
  /* border: 1px solid rgba(255, 255, 255, 0.3); */
  border-radius: 24px;
  touch-action: pan-y;
  background:
    linear-gradient(
      90deg,
      rgba(18, 14, 17, 0.88) 0%,
      rgba(18, 14, 17, 0.58) 58%,
      rgba(18, 14, 17, 0.12) 100%
    ),
    var(--hero-tint, rgb(226 148 100));
  box-shadow: 0 18px 42px rgba(77, 47, 54, 0.2);
}
.hero-track {
  position: absolute;
  inset: 0;
  z-index: 1;
  will-change: transform;
}
.hero-slide {
  position: absolute;
  top: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
  border-radius: inherit;
  pointer-events: none;
}
.hero-slide.is-active {
  pointer-events: auto;
}
.hero-art {
  position: absolute;
  inset: 0 0 0 auto;
  z-index: 0;
  width: var(--hero-art-width);
  height: 100%;
  object-fit: cover;
  object-position: center;
  filter: saturate(1.04);
  -webkit-mask-image: linear-gradient(
    90deg,
    transparent 0%,
    rgba(0, 0, 0, 0.08) 8%,
    rgba(0, 0, 0, 0.42) 17%,
    rgba(0, 0, 0, 0.86) 24%,
    #000 32%
  );
  mask-image: linear-gradient(
    90deg,
    transparent 0%,
    rgba(0, 0, 0, 0.08) 8%,
    rgba(0, 0, 0, 0.42) 17%,
    rgba(0, 0, 0, 0.86) 24%,
    #000 32%
  );
}
.hero-art-placeholder {
  position: absolute;
  top: 50%;
  right: calc(var(--hero-art-width) / 2);
  z-index: 0;
  color: rgba(255, 255, 255, 0.72);
  transform: translate(50%, -50%);
}
.hero-inner {
  position: relative;
  z-index: 2;
  display: grid;
  min-height: 252px;
  grid-template-columns: minmax(0, 1fr) calc(var(--hero-art-width) * 0.7);
  grid-template-rows: 1fr auto;
  gap: 8px 36px;
  padding: 24px 42px 46px;
  border-radius: inherit;
  cursor: pointer;
  outline: none;
}
.hero-inner:focus-visible {
  box-shadow: inset 0 0 0 2px rgba(255, 255, 255, 0.88);
}
.hero-copy {
  display: flex;
  min-width: 0;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding-top: 4px;
}
.hero h1 {
  display: -webkit-box;
  max-width: 760px;
  margin: 0;
  overflow: hidden;
  color: #fff;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: clamp(27px, 2.9vw, 38px);
  line-height: 1.18;
  letter-spacing: 0.02em;
  text-align: center;
  text-wrap: balance;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.hero-author {
  margin: 8px 0 0;
  color: rgba(255, 255, 255, 0.78);
  font-size: 13px;
  letter-spacing: 0.02em;
}
.hero-description {
  display: flex;
  min-width: 0;
  grid-column: 1;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  padding: 8px 18px 9px 13px;
  border-left: 3px solid var(--theme-color);
  background: linear-gradient(
    90deg,
    rgba(20, 17, 19, 0.46) 0%,
    rgba(20, 17, 19, 0.24) 64%,
    rgba(20, 17, 19, 0) 100%
  );
}
.hero-description > p {
  display: -webkit-box;
  margin: 0;
  overflow: hidden;
  color: rgba(255, 255, 255, 0.9);
  font-size: 13px;
  line-height: 1.6;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.hero-meta {
  display: flex;
  flex: 0 0 auto;
  flex-wrap: wrap;
  align-items: center;
  gap: 11px;
  color: rgba(255, 255, 255, 0.72);
  font-size: 12px;
}
.hero-meta > span {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}
.hero-controls {
  position: absolute;
  right: 0;
  bottom: 15px;
  left: 0;
  z-index: 3;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 9px;
}
.hero-arrow {
  display: grid;
  width: 26px;
  height: 26px;
  padding: 0;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 999px;
  color: rgba(255, 255, 255, 0.9);
  background: rgba(255, 255, 255, 0.12);
  place-items: center;
  transition:
    background-color 150ms ease,
    transform 150ms ease;
}
.hero-arrow:hover {
  background: rgba(255, 255, 255, 0.22);
}
.hero-arrow:active {
  transform: scale(0.94);
}
.hero-dots {
  display: flex;
  align-items: center;
  gap: 6px;
}
.hero-dots button {
  width: 6px;
  height: 6px;
  padding: 0;
  border: 0;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.42);
  transition:
    width 180ms ease,
    background-color 180ms ease;
}
.hero-dots button.active {
  width: 20px;
  background: #fff;
}
.hero.dragging {
  cursor: grabbing;
  user-select: none;
  -webkit-user-select: none;
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
.rank-tags {
  display: flex;
  min-height: 20px;
  gap: 4px;
  overflow: hidden;
}
.rank-tags span {
  overflow: hidden;
  padding: 2px 6px;
  border-radius: 5px;
  color: #956653;
  background: rgba(247, 228, 216, 0.86);
  font-size: 12px;
  line-height: 1.35;
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
  --glass-lift:
    0 18px 30px -14px rgba(137, 76, 55, 0.28), 0 2px 3px rgba(137, 76, 55, 0.06);
  transform: translateY(-2px);
}
.recent-card:focus-visible {
  outline: 2px solid var(--theme-color);
  outline-offset: 2px;
}
.recent-cover {
  display: grid;
  width: 64px;
  /* height: 91px; */
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
  display: flex;
  gap: 4px;
  overflow: hidden;
  margin-top: auto;
  color: #b28c7d;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.recent-tag {
  flex: 0 0 auto;
  overflow: hidden;
  padding: 2px 6px;
  border-radius: 5px;
  color: #956653;
  background: rgba(247, 228, 216, 0.86);
  font-size: 11px;
  line-height: 1.35;
  text-overflow: ellipsis;
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
    --hero-art-width: clamp(138px, 43%, 172px);
    min-height: 218px;
    border-radius: 20px;
  }
  .hero-slide {
    min-height: 218px;
  }
  .hero-inner {
    min-height: 218px;
    gap: 8px 10px;
    padding: 16px 16px 40px;
  }
  .hero-copy {
    padding-top: 0;
  }
  .hero h1 {
    font-size: clamp(21px, 6.2vw, 25px);
    line-height: 1.2;
    -webkit-line-clamp: 2;
  }
  .hero-author {
    margin-top: 6px;
    font-size: 12px;
  }
  .hero-description {
    gap: 8px;
    padding: 7px 12px 8px 10px;
    border-left-width: 3px;
  }
  .hero-description > p {
    font-size: 12px;
    line-height: 1.55;
  }
  .hero-meta {
    display: none;
  }
  .hero-controls {
    bottom: 9px;
    gap: 7px;
  }
  .hero-arrow {
    width: 24px;
    height: 24px;
  }
  .hero-dots button {
    width: 5px;
    height: 5px;
  }
  .hero-dots button.active {
    width: 16px;
  }
  .home-loading {
    min-height: 218px;
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
    /* height: 88px; */
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
  .hero-arrow,
  .hero-dots button,
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
