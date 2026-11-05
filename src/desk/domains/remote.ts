import { invoke } from "@tauri-apps/api/core";
import { ref, watch, type Ref } from "vue";
import type {
  AuthStatus,
  BookCommentPage,
  BookCommentReplyPage,
  MediaType,
  Novel,
} from "../../types";
import { currentView, pushView, returnViews } from "../navigation";
import { nativeErrorMessage } from "../../utils/native";
import type { Notify } from "../../composables/useToast";

/** 远端作品按内容形态分组；作品未标注形态时按小说处理。 */
type RemoteMedia = Partial<Record<MediaType, Novel>>;

type RemoteDeskOptions = {
  auth: Ref<AuthStatus>;
  notify: Notify;
  /** 收藏变化后刷新远端书架。 */
  refreshBookshelf: () => Promise<void>;
  /** 离开详情会话时释放在线阅读缓冲。 */
  resetOnlineSession: () => void;
};

function remoteMediaType(novel: Novel): MediaType {
  return novel.bookshelfType || "novel";
}

function normalizeRemoteWorkName(value: string) {
  return value.replace(/\s+/g, "").trim();
}

/** 同一部作品在不同内容形态下的标题与作者可能略有差异。 */
function isSameRemoteWork(candidate: Novel, source: Novel) {
  if (
    normalizeRemoteWorkName(candidate.novelName) !==
    normalizeRemoteWorkName(source.novelName)
  ) {
    return false;
  }
  return (
    !source.authorName ||
    source.authorName === "未知作者" ||
    !candidate.authorName ||
    candidate.authorName === "未知作者" ||
    candidate.authorName === source.authorName
  );
}

function remoteDetailKey(novel: Novel) {
  return `${remoteMediaType(novel)}:${novel.novelId}:${novel.mediaId || ""}:${novel.sourcePath || ""}`;
}

function remoteFavoriteKey(novel: Novel) {
  return `${remoteMediaType(novel)}:${novel.novelId}:${novel.mediaId || ""}`;
}

/**
 * 远端详情会话。
 *
 * 负责单次进入详情页期间的作品、媒体变体、收藏状态与请求去重。会话有明确生命
 * 周期（进入建立、离开清理），因此所有缓存都留在域内，不进入任何全局集合。
 */
export function createRemoteDesk({
  auth,
  notify,
  refreshBookshelf,
  resetOnlineSession,
}: RemoteDeskOptions) {
  const remoteBook = ref<Novel>();
  const remoteBookLoading = ref(false);
  const remoteMedia = ref<RemoteMedia>({});
  const remoteMediaDetecting = ref(false);
  const remoteFavoriteKeys = ref<Set<string>>(new Set());
  const remoteFavoriteLoading = ref("");

  const detailCache = new Map<string, Novel>();
  const detailRequests = new Map<string, Promise<Novel>>();
  let favoritesRequest: Promise<void> | undefined;

  /** 会话是否处于活动状态；与路由无关，进入/离开时显式维护。 */
  let sessionActive = false;
  let bookRequest = 0;
  let generation = 0;

  function isSessionActive() {
    return sessionActive;
  }

  function isRemoteFavorite(novel?: Novel) {
    return novel
      ? remoteFavoriteKeys.value.has(remoteFavoriteKey(novel))
      : false;
  }

  function isRemoteFavoriteLoading(novel?: Novel) {
    return !!novel && remoteFavoriteLoading.value === remoteFavoriteKey(novel);
  }

  function loadRemoteFavorites() {
    if (!auth.value.appAuthenticated)
      return favoritesRequest ?? Promise.resolve();
    if (favoritesRequest) return favoritesRequest;
    favoritesRequest = (async () => {
      try {
        const response = await invoke<{ items: Novel[] }>("get_bookshelf", {
          forceRefresh: false,
        });
        for (const item of response.items)
          remoteFavoriteKeys.value.add(remoteFavoriteKey(item));
      } catch {
        // 未登录或书架读取失败时保持未收藏状态。
      } finally {
        favoritesRequest = undefined;
      }
    })();
    return favoritesRequest;
  }

  async function toggleRemoteFavorite(novel: Novel) {
    const key = remoteFavoriteKey(novel);
    if (remoteFavoriteLoading.value) return;
    const favorited = remoteFavoriteKeys.value.has(key);
    remoteFavoriteLoading.value = key;
    try {
      await invoke(
        favorited ? "remove_remote_favorite" : "add_remote_favorite",
        {
          kind: remoteMediaType(novel),
          novelId: novel.novelId,
          mediaId: novel.mediaId,
        },
      );
      if (favorited) {
        remoteFavoriteKeys.value.delete(key);
        notify("已取消收藏");
      } else {
        remoteFavoriteKeys.value.add(key);
        notify("已收藏");
      }
      void refreshBookshelf();
    } catch (error) {
      notify(
        nativeErrorMessage(error, favorited ? "取消收藏失败" : "收藏失败"),
        "error",
      );
    } finally {
      if (remoteFavoriteLoading.value === key) remoteFavoriteLoading.value = "";
    }
  }

  watch(
    () => auth.value.appAuthenticated,
    (authenticated) => {
      remoteFavoriteKeys.value = new Set();
      remoteFavoriteLoading.value = "";
      if (authenticated && currentView() === "remoteDetail")
        void loadRemoteFavorites();
    },
  );

  function requestDetail(
    novel: Novel,
    kind: MediaType = remoteMediaType(novel),
  ): Promise<Partial<Novel> | undefined> {
    if (kind === "comic") {
      return invoke<Partial<Novel>>("get_comic_details", {
        comicId: novel.novelId,
      });
    }
    if (kind === "audio") {
      return novel.mediaId
        ? invoke<Partial<Novel>>("get_audio_details", {
            albumId: novel.mediaId,
            novelId: novel.novelId,
          })
        : Promise.resolve(undefined);
    }
    return invoke<Partial<Novel>>("get_novel_details", {
      novelId: novel.novelId,
    });
  }

  function fetchDetail(novel: Novel, requestGeneration: number) {
    const key = remoteDetailKey(novel);
    const cached = detailCache.get(key);
    if (cached) return Promise.resolve(cached);
    const pending = detailRequests.get(key);
    if (pending) return pending;
    const request = (async () => {
      const details = await requestDetail(novel);
      const hydrated = details ? { ...novel, ...details } : novel;
      if (requestGeneration === generation) detailCache.set(key, hydrated);
      return hydrated;
    })();
    detailRequests.set(key, request);
    const settle = () => {
      if (detailRequests.get(key) === request) detailRequests.delete(key);
    };
    request.then(settle, settle);
    return request;
  }

  /** 章节选择弹窗复用详情缓存时的解析入口。 */
  async function resolveDetail(
    novel: Novel,
    kind: "novel" | "comic",
  ): Promise<Partial<Novel> | undefined> {
    if (
      sessionActive &&
      remoteBook.value &&
      remoteDetailKey(remoteBook.value) === remoteDetailKey(novel)
    ) {
      return remoteBook.value;
    }
    const details = await requestDetail(novel, kind);
    return details ? { ...novel, ...details } : novel;
  }

  /**
   * 作品评论 / 书评分页。
   *
   * 评论页与详情页共用同一组上游参数，集中到这里后页面只负责分页状态。
   */
  function fetchComments(query: {
    kind: MediaType;
    novelId: number;
    mediaId?: number;
    reviews: boolean;
    page: number;
    size: number;
  }) {
    return invoke<BookCommentPage>("get_book_comments", {
      kind: query.kind,
      novelId: query.novelId,
      mediaId: query.mediaId ?? null,
      page: query.page,
      size: query.size,
      reviews: query.reviews,
    });
  }

  /** 单条评论的回复分页。 */
  function fetchReplies(query: {
    kind: MediaType;
    commentId: number;
    reviews: boolean;
    page: number;
    size: number;
  }) {
    return invoke<BookCommentReplyPage>("get_comment_replies", {
      kind: query.kind,
      commentId: query.commentId,
      reviews: query.reviews,
      page: query.page,
      size: query.size,
    });
  }

  function clearSession() {
    sessionActive = false;
    bookRequest += 1;
    generation += 1;
    remoteBookLoading.value = false;
    detailCache.clear();
    detailRequests.clear();
    resetOnlineSession();
  }

  /**
   * 打开远端作品详情。
   * @param preserveMedia 切换内容形态时保留当前会话与缓存。
   */
  async function open(novel: Novel, preserveMedia = false) {
    if (!preserveMedia && currentView() !== "remoteDetail") {
      returnViews.remoteDetail.value = currentView();
    }
    if (!preserveMedia) {
      clearSession();
      sessionActive = true;
    }
    const requestGeneration = preserveMedia ? generation : ++generation;
    const request = ++bookRequest;
    remoteBook.value = novel;
    if (!preserveMedia) {
      void pushView("remoteDetail");
      void loadRemoteFavorites();
      remoteMedia.value = { [remoteMediaType(novel)]: novel };
      remoteMediaDetecting.value = true;
    } else {
      remoteMediaDetecting.value = false;
    }
    const cached = detailCache.get(remoteDetailKey(novel));
    remoteBookLoading.value = !cached;
    try {
      const hydrated = await fetchDetail(novel, requestGeneration);
      if (request === bookRequest) {
        remoteBook.value = hydrated;
        remoteMedia.value = {
          ...remoteMedia.value,
          [remoteMediaType(novel)]: remoteBook.value,
        };
      }
    } catch (error) {
      if (request === bookRequest)
        notify(nativeErrorMessage(error, "读取作品详情失败"), "error");
    } finally {
      if (request === bookRequest) remoteBookLoading.value = false;
    }

    if (preserveMedia) return;
    try {
      const candidates = await invoke<Novel[]>("search_novels", {
        query: novel.novelName,
      });
      if (request !== bookRequest) return;
      const detected: RemoteMedia = { ...remoteMedia.value };
      for (const candidate of candidates) {
        if (isSameRemoteWork(candidate, novel)) {
          detected[remoteMediaType(candidate)] = candidate;
        }
      }
      remoteMedia.value = detected;
    } catch {
      // 媒体变体探测是可选增强，失败时详情仍可用。
    } finally {
      if (request === bookRequest) remoteMediaDetecting.value = false;
    }
  }

  function selectMedia(media: MediaType) {
    const target = remoteMedia.value[media];
    if (target) void open(target, true);
  }

  function close() {
    const target = returnViews.remoteDetail.value;
    clearSession();
    void pushView(target);
  }

  return {
    remoteBook,
    remoteBookLoading,
    remoteMedia,
    remoteMediaDetecting,
    isRemoteFavorite,
    isRemoteFavoriteLoading,
    toggleRemoteFavorite,
    openRemoteBookDetail: open,
    closeRemoteBookDetail: close,
    selectRemoteMedia: selectMedia,
    resolveDetail,
    fetchComments,
    fetchReplies,
    isSessionActive,
    clearSession,
  };
}

export type RemoteDesk = ReturnType<typeof createRemoteDesk>;
