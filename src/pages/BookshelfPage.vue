<script setup lang="ts">
import { computed, inject, nextTick, ref } from "vue";
import {
  BookMarked,
  ChevronLeft,
  ChevronRight,
  LoaderCircle,
  RefreshCw,
  Search,
  X,
} from "@lucide/vue";
import NovelCard from "../components/NovelCard.vue";
import PageFrame from "../components/PageFrame.vue";
import { deskInjectionKey } from "../deskContext";

function requireDesk() {
  const desk = inject(deskInjectionKey);
  if (!desk) throw new Error("Desk context is unavailable");
  return desk;
}

const desk = requireDesk();

const searchOpen = ref(false);
const shelfQuery = ref("");
const searchInput = ref<HTMLInputElement>();
const normalizedQuery = computed(() =>
  shelfQuery.value.trim().toLocaleLowerCase(),
);
const searchResults = computed(() => {
  const query = normalizedQuery.value;
  if (!query) return [];
  return desk.filteredBookshelf.value.filter((novel) =>
    `${novel.novelName}\n${novel.authorName}`
      .toLocaleLowerCase()
      .includes(query),
  );
});
const visibleBookshelf = computed(() =>
  normalizedQuery.value ? searchResults.value : desk.pagedBookshelf.value,
);

function openSearch() {
  searchOpen.value = true;
  void nextTick(() => searchInput.value?.focus());
}

function closeSearch() {
  searchOpen.value = false;
}

function clearSearch() {
  shelfQuery.value = "";
}

function toggleSearch() {
  if (searchOpen.value) closeSearch();
  else openSearch();
}

function dismissSearch(event: MouseEvent) {
  const target = event.target;
  if (!searchOpen.value || !(target instanceof Element)) return;
  if (target.closest(".shelf-search, .shelf-actions")) return;
  closeSearch();
}

// 书架分页和分类筛选由父级计算，页面只负责转发用户操作。
/**
 * 读取页码输入并将其限制在书架有效页码范围。
 * @param event 页码输入框的变更事件。
 * @returns 无返回值。
 */
function jumpToPage(event: Event) {
  const input = event.target as HTMLInputElement;
  const requestedPage = Number(input.value);
  const page = Number.isInteger(requestedPage)
    ? Math.min(Math.max(requestedPage, 1), desk.bookshelfTotalPages.value)
    : desk.bookshelfPage.value;
  input.value = String(page);
  desk.setBookshelfPage(page);
}
</script>

<template>
  <PageFrame>
    <main class="bookshelf-page" @click="dismissSearch">
      <section class="section-head shelf-head">
        <div>
          <p class="eyebrow">SFACG BOOKSHELF</p>
          <h2>
            {{
              desk.bookshelfLoading.value
                ? "正在同步书架"
                : normalizedQuery
                  ? `找到 ${searchResults.length} 本收藏`
                  : `共 ${desk.bookshelf.value.length} 本收藏`
            }}
          </h2>
        </div>
        <div class="shelf-actions">
          <button
            class="icon-button"
            :class="{ active: searchOpen || normalizedQuery }"
            title="搜索书架"
            aria-label="搜索书架"
            aria-controls="bookshelf-search"
            :aria-expanded="searchOpen"
            @click="toggleSearch"
          >
            <Search :size="19" />
          </button>
          <button
            class="icon-button"
            title="刷新书架"
            :disabled="desk.bookshelfLoading.value"
            @click="desk.refreshBookshelf()"
          >
            <RefreshCw
              :class="{ spin: desk.bookshelfLoading.value }"
              :size="19"
            />
          </button>
        </div>
        <Transition name="search-pop">
          <div
            v-if="searchOpen"
            id="bookshelf-search"
            class="shelf-search"
            role="search"
          >
            <Search :size="16" />
            <input
              ref="searchInput"
              v-model="shelfQuery"
              autocomplete="off"
              placeholder="搜索书名或作者"
              aria-label="搜索书架中的书籍"
              @keydown.esc.prevent="closeSearch"
            />
            <button
              class="clear-search"
              :class="{ visible: shelfQuery }"
              type="button"
              title="清除搜索"
              aria-label="清除搜索"
              :disabled="!shelfQuery"
              @click="clearSearch"
            >
              <X :size="14" />
            </button>
          </div>
        </Transition>
      </section>
      <div
        v-if="desk.bookshelfLoading.value && !desk.bookshelf.value.length"
        class="empty-state"
      >
        <LoaderCircle class="spin" :size="28" />
        <p>正在读取书架</p>
      </div>
      <template v-else-if="desk.bookshelf.value.length">
        <div class="bookshelf-toolbar" aria-label="书架分类">
          <button
            v-for="category in desk.bookshelfCategories.value"
            :key="category"
            class="category-tab"
            :class="{ active: desk.bookshelfCategory.value === category }"
            @click="desk.selectBookshelfCategory(category)"
          >
            {{ category }}
          </button>
        </div>
        <div v-if="visibleBookshelf.length" class="novel-grid shelf-grid">
          <NovelCard
            v-for="novel in visibleBookshelf"
            :key="`${novel.novelId}-${novel.mediaId || ''}-${novel.bookshelfName || '默认书架'}-${novel.bookshelfType || 'novel'}`"
            :novel="novel"
            :format-date="desk.formatDate"
            @select="desk.openChapterPicker"
            @detail="desk.openRemoteBookDetail"
          />
        </div>
        <div v-else class="empty-state">
          <Search v-if="normalizedQuery" :size="28" />
          <BookMarked v-else :size="28" />
          <p>
            {{
              normalizedQuery
                ? "没有找到匹配的书籍"
                : "该分类中还没有可显示的小说"
            }}
          </p>
        </div>
        <nav
          v-if="!normalizedQuery && desk.bookshelfTotalPages.value > 1"
          class="pagination"
          aria-label="书架分页"
        >
          <button
            class="icon-button"
            title="上一页"
            :disabled="desk.bookshelfPage.value === 1"
            @click="desk.setBookshelfPage(desk.bookshelfPage.value - 1)"
          >
            <ChevronLeft :size="18" /></button
          ><span class="page-status">第</span
          ><input
            class="page-jump"
            type="number"
            inputmode="numeric"
            :value="desk.bookshelfPage.value"
            min="1"
            :max="desk.bookshelfTotalPages.value"
            step="1"
            aria-label="输入页码跳转"
            title="输入页码后按回车或移开焦点跳转"
            @change="jumpToPage"
            @keydown.enter.prevent="jumpToPage"
          /><span class="page-status"
            >/ {{ desk.bookshelfTotalPages.value }} 页</span
          ><button
            class="icon-button"
            title="下一页"
            :disabled="
              desk.bookshelfPage.value === desk.bookshelfTotalPages.value
            "
            @click="desk.setBookshelfPage(desk.bookshelfPage.value + 1)"
          >
            <ChevronRight :size="18" />
          </button>
        </nav>
      </template>
      <div v-else class="empty-state tall">
        <BookMarked :size="32" />
        <p>书架中还没有可显示的作品</p>
      </div>
    </main>
  </PageFrame>
</template>

<style scoped>
.bookshelf-page {
  min-width: 0;
  height: 100%;
  padding: 0px 42px 28px 18px;
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
.shelf-head {
  position: relative;
  margin-top: 7px;
}
.shelf-actions {
  display: flex;
  gap: 8px;
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
.icon-button {
  display: grid;
  width: 40px;
  height: 40px;
  border: 0;
  border-radius: 13px;
  color: #a17367;
  background: rgba(255, 255, 255, 0.7);
  place-items: center;
  transition: 0.2s;
}
.icon-button:hover {
  color: var(--theme-color-dark);
  background: #fff0e7;
}
.icon-button:focus-visible {
  outline: 2px solid var(--theme-color);
  outline-offset: 2px;
}
.icon-button.active,
.icon-button.active:hover {
  color: #fff;
  background: var(--theme-color);
}
.shelf-search {
  position: absolute;
  top: calc(100% + 9px);
  left: 50%;
  z-index: 20;
  display: flex;
  align-items: center;
  gap: 8px;
  width: min(440px, calc(100% - 8px));
  padding: 8px 8px 8px 12px;
  border: 1px solid #f1d9ce;
  border-radius: 13px;
  color: #b17f70;
  background: rgba(255, 252, 249, 0.97);
  box-shadow: 0 16px 34px rgba(115, 72, 56, 0.18);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
  transform: translateX(-50%);
  transform-origin: top center;
}
.shelf-search:focus-within {
  border-color: var(--theme-color);
  box-shadow:
    0 16px 34px rgba(115, 72, 56, 0.18),
    0 0 0 3px rgba(226, 148, 100, 0.14);
}
.shelf-search input {
  flex: 1;
  min-width: 0;
  border: 0;
  outline: 0;
  color: #5f423c;
  background: transparent;
  font-size: 13px;
}
.shelf-search input::placeholder {
  color: #bd978a;
}
.clear-search {
  display: grid;
  flex: none;
  width: 24px;
  height: 24px;
  padding: 0;
  border: 0;
  border-radius: 7px;
  color: #a17367;
  background: transparent;
  place-items: center;
  visibility: hidden;
  transition: 0.15s;
}
.clear-search.visible {
  visibility: visible;
}
.clear-search:hover {
  color: var(--theme-color-dark);
  background: #ffeadf;
}
.search-pop-enter-active {
  transition:
    opacity 140ms ease-out,
    transform 220ms linear(0, 0.58 18%, 0.89 38%, 1.03 62%, 0.995 82%, 1);
}
.search-pop-leave-active {
  transition:
    opacity 140ms ease-in,
    transform 140ms ease-in;
}
.search-pop-enter-from,
.search-pop-leave-to {
  opacity: 0;
  transform: translate(-50%, -6px) scale(0.97);
}
.search-pop-enter-to,
.search-pop-leave-from {
  opacity: 1;
  transform: translate(-50%, 0) scale(1);
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
.bookshelf-toolbar {
  display: flex;
  gap: 7px;
  padding: 2px 1px 12px;
  overflow-x: auto;
  scrollbar-width: none;
}
.category-tab {
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
.category-tab:hover,
.category-tab.active {
  border-color: var(--theme-color);
  color: #fff;
  background: var(--theme-color);
}
.novel-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}
.pagination {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  margin: 22px 0 4px;
  color: #a27a6f;
  font-size: 12px;
}
.pagination .icon-button {
  width: 32px;
  height: 32px;
}
.pagination .icon-button:disabled {
  cursor: not-allowed;
  opacity: 0.4;
}
.page-jump {
  width: 48px;
  min-height: 32px;
  padding: 0 5px;
  border: 1px solid #efd3c5;
  border-radius: 7px;
  color: #704c43;
  background: rgba(255, 250, 247, 0.8);
  font-size: 12px;
  text-align: center;
  outline: 0;
}
.page-jump:focus {
  border-color: var(--theme-color);
  box-shadow: 0 0 0 2px rgba(226, 148, 100, 0.18);
}
.page-jump::-webkit-inner-spin-button,
.page-jump::-webkit-outer-spin-button {
  margin: 0;
}

@media (max-width: 760px) {
  .bookshelf-page {
    height: auto;
    padding: 10px 0 0;
    overflow: visible;
  }
  .novel-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 18px 12px;
  }
  .shelf-grid :deep(.novel-card) {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 7px;
    min-height: 0;
    padding: 0;
    border: 0;
    border-radius: 0;
    background: transparent;
    box-shadow: none;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }
  .shelf-grid :deep(.novel-card:hover) {
    box-shadow: none;
    transform: none;
  }
  .shelf-grid :deep(.novel-card img),
  .shelf-grid :deep(.novel-cover-placeholder) {
    width: 100%;
    height: auto;
    aspect-ratio: 3 / 4;
    border-radius: 10px;
  }
  .shelf-grid :deep(.novel-card img) {
    display: block;
    object-fit: cover;
  }
  .shelf-grid :deep(.novel-cover-placeholder svg) {
    width: 22px;
    height: 22px;
  }
  .shelf-grid :deep(.novel-info) {
    display: block;
    height: auto;
  }
  .shelf-grid :deep(.novel-name) {
    min-height: 2.7em;
    font-size: 13px;
    line-height: 1.35;
  }
  .shelf-grid :deep(.author) {
    overflow: hidden;
    margin-top: 2px;
    font-size: 12px;
    line-height: 1.35;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .shelf-grid :deep(.media-type) {
    position: absolute;
    top: 7px;
    left: 7px;
    z-index: 2;
    display: grid;
    width: 28px;
    height: 28px;
    margin: 0;
    padding: 0;
    border-radius: 8px;
    box-shadow: 0 3px 10px rgba(94, 61, 49, 0.16);
    gap: 0;
    place-items: center;
  }
  .shelf-grid :deep(.media-type svg) {
    width: 16px;
    height: 16px;
  }
  .shelf-grid :deep(.media-type-audio) {
    color: #a6535c;
    background: #fdf1f0;
  }
  .shelf-grid :deep(.media-type-comic) {
    color: #8b6a9b;
    background: #faf4fc;
  }
  .shelf-grid :deep(.media-type-novel) {
    display: none;
  }
  .shelf-grid :deep(.media-type-label) {
    display: none;
  }
  .shelf-grid :deep(.updated),
  .shelf-grid :deep(.novel-actions) {
    display: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .shelf-search,
  .search-pop-enter-active,
  .search-pop-leave-active {
    transition: none;
  }
}
</style>
