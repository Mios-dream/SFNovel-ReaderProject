import { invoke } from "@tauri-apps/api/core";
import { computed, reactive, ref, watch, type ComputedRef } from "vue";
import type { BookComment, BookCommentPage, Novel } from "../types";
import { nativeErrorMessage } from "./deskShared";

export type DiscussionTab = "comments" | "reviews";

type DiscussionState = {
  items: BookComment[];
  highlights: BookComment[];
  page: number;
  hasMore: boolean;
  loaded: boolean;
  loading: boolean;
  loadingMore: boolean;
  error: string;
};

function emptyDiscussion(): DiscussionState {
  return {
    items: [],
    highlights: [],
    page: -1,
    hasMore: false,
    loaded: false,
    loading: false,
    loadingMore: false,
    error: "",
  };
}

export function useBookDiscussions(
  novel: ComputedRef<Novel | undefined>,
  pageSize: number,
  initialTab: DiscussionTab = "comments",
) {
  const activeTab = ref<DiscussionTab>(initialTab);
  const states = reactive<Record<DiscussionTab, DiscussionState>>({
    comments: emptyDiscussion(),
    reviews: emptyDiscussion(),
  });
  const requests: Record<DiscussionTab, number> = {
    comments: 0,
    reviews: 0,
  };
  const current = computed(() => states[activeTab.value]);
  const preview = computed(() => current.value.items.slice(0, 3));
  const highlights = computed(() => states.comments.highlights);

  async function load(tab: DiscussionTab, reset = false) {
    const work = novel.value;
    if (!work) return;
    const state = states[tab];
    if (!reset && (!state.loaded || state.loading || state.loadingMore || !state.hasMore))
      return;
    const request = ++requests[tab];
    const page = reset ? 0 : state.page + 1;
    if (reset) {
      state.loading = true;
      state.error = "";
    } else {
      state.loadingMore = true;
      state.error = "";
    }
    try {
      const result = await invoke<BookCommentPage>("get_book_comments", {
        kind: work.bookshelfType || "novel",
        novelId: work.novelId,
        mediaId: work.mediaId ?? null,
        page,
        size: pageSize,
        reviews: tab === "reviews",
      });
      if (request !== requests[tab]) return;
      const known = new Set(state.items.map((comment) => comment.commentId));
      state.items = reset
        ? result.comments
        : [
            ...state.items,
            ...result.comments.filter((comment) => !known.has(comment.commentId)),
          ];
      const knownHighlights = new Set(
        state.highlights.map((comment) => comment.commentId),
      );
      state.highlights = reset
        ? result.highlights || []
        : [
            ...state.highlights,
            ...(result.highlights || []).filter(
              (comment) => !knownHighlights.has(comment.commentId),
            ),
          ];
      state.page = result.page;
      state.hasMore = result.hasMore;
      state.loaded = true;
    } catch (error) {
      if (request === requests[tab])
        state.error = nativeErrorMessage(
          error,
          tab === "reviews" ? "书评加载失败" : "评论加载失败",
        );
    } finally {
      if (request === requests[tab]) {
        state.loading = false;
        state.loadingMore = false;
      }
    }
  }

  function selectTab(tab: DiscussionTab) {
    activeTab.value = tab;
    if (!states[tab].loaded) void load(tab, true);
    if (tab === "reviews" && !states.comments.loaded && !states.comments.loading)
      void load("comments", true);
  }

  function loadMore() {
    void load(activeTab.value, false);
  }

  function reload() {
    void load(activeTab.value, true);
  }

  watch(
    () =>
      [
        novel.value?.novelId,
        novel.value?.mediaId,
        novel.value?.bookshelfType,
      ] as const,
    () => {
      for (const tab of ["comments", "reviews"] as const) {
        requests[tab] += 1;
        Object.assign(states[tab], emptyDiscussion());
      }
      void load(activeTab.value, true);
      if (activeTab.value === "reviews") void load("comments", true);
    },
    { immediate: true },
  );

  return {
    activeTab,
    current,
    preview,
    highlights,
    selectTab,
    loadMore,
    reload,
  };
}
