<script setup lang="ts">
import { inject } from "vue";
import {
  BookMarked,
  ChevronLeft,
  ChevronRight,
  LoaderCircle,
  RefreshCw,
} from "lucide-vue-next";
import NovelCard from "../components/NovelCard.vue";
import PageFrame from "../components/PageFrame.vue";
import { deskInjectionKey } from "../deskContext";

function requireDesk() {
  const desk = inject(deskInjectionKey);
  if (!desk) throw new Error("Desk context is unavailable");
  return desk;
}

const desk = requireDesk();

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
    <main class="bookshelf-page">
      <section class="section-head shelf-head">
        <div>
          <p class="eyebrow">SFACG BOOKSHELF</p>
          <h2>
            {{
              desk.bookshelfLoading.value
                ? "正在同步书架"
                : `共 ${desk.bookshelf.value.length} 本收藏`
            }}
          </h2>
        </div>
        <button
          class="icon-button"
          title="刷新书架"
          :disabled="desk.bookshelfLoading.value"
          @click="desk.refreshBookshelf(true)"
        >
          <RefreshCw
            :class="{ spin: desk.bookshelfLoading.value }"
            :size="19"
          />
        </button>
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
        <div
          v-if="desk.pagedBookshelf.value.length"
          class="novel-grid shelf-grid"
        >
          <NovelCard
            v-for="novel in desk.pagedBookshelf.value"
            :key="`${novel.novelId}-${novel.bookshelfName || '默认书架'}-${novel.bookshelfType || 'novel'}`"
            :novel="novel"
            :format-date="desk.formatDate"
            @select="desk.openChapterPicker"
          />
        </div>
        <div v-else class="empty-state">
          <BookMarked :size="28" />
          <p>该分类中还没有可显示的小说</p>
        </div>
        <nav
          v-if="desk.bookshelfTotalPages.value > 1"
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
            >/ {{ desk.bookshelfTotalPages.value }} 页 ·
            {{ desk.filteredBookshelf.value.length }} 本</span
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
  scrollbar-width: thin;
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
    grid-template-columns: 1fr;
  }
}
</style>
