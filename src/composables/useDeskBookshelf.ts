import { invoke } from "@tauri-apps/api/core";
import { computed, ref, type Ref } from "vue";
import type { AuthStatus, Novel } from "../types";
import { nativeErrorMessage, type Notify } from "./deskShared";

type BookshelfResponse = { categories: string[]; items: Novel[] };
type UseDeskBookshelfOptions = {
  auth: Ref<AuthStatus>;
  refreshAuthStatus: () => Promise<void>;
  requestCredentials: () => void;
  notify: Notify;
};

export function useDeskBookshelf({
  auth,
  refreshAuthStatus,
  requestCredentials,
  notify,
}: UseDeskBookshelfOptions) {
  const bookshelf = ref<Novel[]>([]);
  const bookshelfGroupNames = ref<string[]>([]);
  const bookshelfLoading = ref(false);
  const bookshelfCategory = ref("全部");
  const bookshelfPage = ref(1);
  const bookshelfPageSize = 12;

  const bookshelfCategoryFor = (novel: Novel) =>
    novel.bookshelfName || "未分类";
  const bookshelfCategories = computed(() => [
    "全部",
    ...Array.from(
      new Set([
        ...bookshelfGroupNames.value,
        ...bookshelf.value.map(bookshelfCategoryFor),
      ]),
    ),
  ]);
  const filteredBookshelf = computed(() =>
    bookshelfCategory.value === "全部"
      ? bookshelf.value
      : bookshelf.value.filter(
          (novel) => bookshelfCategoryFor(novel) === bookshelfCategory.value,
        ),
  );
  const bookshelfTotalPages = computed(() =>
    Math.max(1, Math.ceil(filteredBookshelf.value.length / bookshelfPageSize)),
  );
  const pagedBookshelf = computed(() =>
    filteredBookshelf.value.slice(
      (bookshelfPage.value - 1) * bookshelfPageSize,
      bookshelfPage.value * bookshelfPageSize,
    ),
  );

  async function refreshBookshelf(forceRefresh = false) {
    await refreshAuthStatus();
    if (!auth.value.authenticated) {
      requestCredentials();
      notify("登录后即可读取 SF 书架");
      return;
    }
    bookshelfLoading.value = true;
    try {
      const response = await invoke<BookshelfResponse>("get_bookshelf", {
        forceRefresh,
      });
      bookshelf.value = response.items;
      bookshelfGroupNames.value = response.categories;
      if (!bookshelfCategories.value.includes(bookshelfCategory.value))
        bookshelfCategory.value = "全部";
      bookshelfPage.value = 1;
    } catch (error) {
      notify(nativeErrorMessage(error, "读取书架失败"));
    } finally {
      bookshelfLoading.value = false;
    }
  }

  function selectBookshelfCategory(category: string) {
    bookshelfCategory.value = category;
    bookshelfPage.value = 1;
  }

  function setBookshelfPage(page: number) {
    bookshelfPage.value = Math.min(
      Math.max(page, 1),
      bookshelfTotalPages.value,
    );
  }

  return {
    bookshelf,
    bookshelfLoading,
    bookshelfCategory,
    bookshelfPage,
    bookshelfCategories,
    filteredBookshelf,
    bookshelfTotalPages,
    pagedBookshelf,
    refreshBookshelf,
    selectBookshelfCategory,
    setBookshelfPage,
  };
}
