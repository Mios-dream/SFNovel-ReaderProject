import { onBackButtonPress } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { onBeforeUnmount, onMounted, ref } from "vue";
import type { ChapterMode, Novel, ReaderSource, ViewName } from "../types";
import { useChapterPicker } from "./useChapterPicker";
import { useDeskAuth } from "./useDeskAuth";
import { useDeskBookshelf } from "./useDeskBookshelf";
import { useDeskJobs } from "./useDeskJobs";
import { useDeskLibrary } from "./useDeskLibrary";
import { useNovelSearch } from "./useNovelSearch";
import { useOnlineReading } from "./useOnlineReading";
import { useRequestPolicy } from "./useRequestPolicy";
import { showToast, type ToastType } from "./useToast";

type RemoteMediaType = "novel" | "audio" | "comic";
type RemoteMedia = Partial<Record<RemoteMediaType, Novel>>;

function remoteMediaType(novel: Novel): RemoteMediaType {
  return novel.bookshelfType || "novel";
}

function normalizeRemoteWorkName(value: string) {
  return value.replace(/\s+/g, "").trim();
}

function isSameRemoteWork(candidate: Novel, source: Novel) {
  if (
    normalizeRemoteWorkName(candidate.novelName) !==
    normalizeRemoteWorkName(source.novelName)
  ) {
    return false;
  }
  return !source.authorName ||
    source.authorName === "未知作者" ||
    !candidate.authorName ||
    candidate.authorName === "未知作者" ||
    candidate.authorName === source.authorName;
}

/**
 * Composes the desktop's domain modules and owns cross-domain navigation.
 * Individual API calls and domain state deliberately live in their modules.
 */
export function useNovelDesk() {
  const active = ref<ViewName>("discover");
  const readerSource = ref<ReaderSource>("local");
  const libraryDetailReturnView = ref<ViewName>("library");
  const remoteDetailReturnView = ref<ViewName>("bookshelf");
  const onlineReturnView = ref<ViewName>("remoteDetail");
  const queueOpen = ref(false);
  const remoteBook = ref<Novel>();
  const remoteBookLoading = ref(false);
  const remoteMedia = ref<RemoteMedia>({});
  const remoteMediaDetecting = ref(false);
  const remoteDetailCache = new Map<string, Novel>();
  const remoteDetailRequests = new Map<string, Promise<Novel>>();
  let stopBackListener: { unregister: () => Promise<void> } | undefined;
  let disposed = false;
  let remoteBookRequest = 0;
  let remoteDetailSessionActive = false;
  let remoteDetailGeneration = 0;

  function notify(message: string, type: ToastType = "info") {
    showToast(message, type);
  }

  const auth = useDeskAuth(notify);

  function requireAudioAccess() {
    if (auth.auth.value.appAuthenticated) return true;
    auth.openAppCredentials();
    notify("有声内容需要连接 App 凭证");
    return false;
  }

  const requestPolicy = useRequestPolicy(notify);
  const library = useDeskLibrary({
    libraryDetailReturnView,
    notify,
  });
  const jobs = useDeskJobs({
    notify,
    refreshLibrary: library.refreshLibrary,
    refreshLocalBook: library.refreshLocalBook,
  });
  const search = useNovelSearch({
    refreshAuthStatus: auth.refreshAuthStatus,
    notify,
  });
  const bookshelf = useDeskBookshelf({
    auth: auth.auth,
    refreshAuthStatus: auth.refreshAuthStatus,
    requestCredentials: () => {
      auth.openCredentials();
    },
    notify,
  });
  const online = useOnlineReading({ notify });
  const chapterPicker = useChapterPicker({
    auth: auth.auth,
    notify,
    addJob: jobs.upsertJob,
    refreshJobs: jobs.refreshJobs,
    loadCatalog: async (novel, mode) => {
      const snapshot = await online.loadDirectorySnapshot(
        novel,
        mode,
        false,
        remoteDetailSessionActive,
      );
      return mode === "text"
        ? { volumes: snapshot.textVolumes }
        : {
            chapters:
              mode === "audio"
                ? snapshot.audioChapters
                : snapshot.comicChapters,
          };
    },
    loadDetail: async (novel, kind) => {
      if (
        remoteDetailSessionActive &&
        remoteBook.value &&
        remoteDetailKey(remoteBook.value) === remoteDetailKey(novel)
      ) {
        return remoteBook.value;
      }
      const details = await requestRemoteDetail(
        novel,
        kind === "comic" ? "comic" : "novel",
      );
      return details ? { ...novel, ...details } : novel;
    },
  });

  function remoteDetailKey(novel: Novel) {
    return `${remoteMediaType(novel)}:${novel.novelId}:${novel.mediaId || ""}:${novel.sourcePath || ""}`;
  }

  function requestRemoteDetail(
    novel: Novel,
    kind: RemoteMediaType = remoteMediaType(novel),
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

  function fetchRemoteDetail(novel: Novel, generation: number) {
    const key = remoteDetailKey(novel);
    const cached = remoteDetailCache.get(key);
    if (cached) return Promise.resolve(cached);
    const pending = remoteDetailRequests.get(key);
    if (pending) return pending;
    const request = (async () => {
      const details = await requestRemoteDetail(novel);
      const hydrated = details ? { ...novel, ...details } : novel;
      if (generation === remoteDetailGeneration)
        remoteDetailCache.set(key, hydrated);
      return hydrated;
    })();
    remoteDetailRequests.set(key, request);
    request.then(
      () => {
        if (remoteDetailRequests.get(key) === request)
          remoteDetailRequests.delete(key);
      },
      () => {
        if (remoteDetailRequests.get(key) === request)
          remoteDetailRequests.delete(key);
      },
    );
    return request;
  }

  function clearRemoteDetailSession() {
    remoteDetailSessionActive = false;
    remoteBookRequest += 1;
    remoteDetailGeneration += 1;
    remoteBookLoading.value = false;
    remoteDetailCache.clear();
    remoteDetailRequests.clear();
    online.resetOnlineReadingSession();
  }

  async function openRemoteBookDetail(novel: Novel, preserveMedia = false) {
    if (remoteMediaType(novel) === "audio" && !requireAudioAccess()) return;
    if (!preserveMedia && active.value !== "remoteDetail") {
      remoteDetailReturnView.value = active.value;
    }
    if (!preserveMedia) {
      clearRemoteDetailSession();
      remoteDetailSessionActive = true;
    }
    const generation = preserveMedia
      ? remoteDetailGeneration
      : ++remoteDetailGeneration;
    const request = ++remoteBookRequest;
    remoteBook.value = novel;
    active.value = "remoteDetail";
    if (!preserveMedia) {
      remoteMedia.value = { [remoteMediaType(novel)]: novel };
      remoteMediaDetecting.value = true;
    } else {
      remoteMediaDetecting.value = false;
    }
    const cached = remoteDetailCache.get(remoteDetailKey(novel));
    remoteBookLoading.value = !cached;
    try {
      const hydrated = await fetchRemoteDetail(novel, generation);
      if (request === remoteBookRequest) {
        remoteBook.value = hydrated;
        remoteMedia.value = {
          ...remoteMedia.value,
          [remoteMediaType(novel)]: remoteBook.value,
        };
      }
    } catch (error) {
      if (request === remoteBookRequest)
        notify(error instanceof Error ? error.message : "读取作品详情失败", "error");
    } finally {
      if (request === remoteBookRequest) remoteBookLoading.value = false;
    }

    if (preserveMedia) return;
    try {
      const candidates = await invoke<Novel[]>("search_novels", {
        query: novel.novelName,
      });
      if (request !== remoteBookRequest) return;
      const detected: RemoteMedia = { ...remoteMedia.value };
      for (const candidate of candidates) {
        if (isSameRemoteWork(candidate, novel)) {
          detected[remoteMediaType(candidate)] = candidate;
        }
      }
      remoteMedia.value = detected;
    } catch {
      // Detail remains usable when the optional media-variant probe is unavailable.
    } finally {
      if (request === remoteBookRequest) remoteMediaDetecting.value = false;
    }
  }

  function selectRemoteMedia(media: RemoteMediaType) {
    const target = remoteMedia.value[media];
    if (target) void openRemoteBookDetail(target, true);
  }

  function closeRemoteBookDetail() {
    clearRemoteDetailSession();
    active.value = remoteDetailReturnView.value;
  }

  function startOnlineReading(
    novel: Novel,
    mode: ChapterMode,
    chapterId?: number,
  ) {
    if (mode === "audio" && !requireAudioAccess()) return;
    if (
      active.value !== "reader" &&
      active.value !== "comicReader" &&
      active.value !== "audioPlayer"
    ) {
      onlineReturnView.value = active.value;
    }
    online.prepareOnlineReading(novel, mode, chapterId);
    readerSource.value = "online";
    active.value =
      mode === "audio"
        ? "audioPlayer"
        : mode === "comic"
          ? "comicReader"
          : "reader";
  }

  function startOnlineFromLibrary(mode: "novel" | "audio" | "comic") {
    const book = library.localBook.value;
    const work = book?.[mode];
    if (!work?.id) return;
    startOnlineReading(
      {
        novelId:
          mode === "audio" ? work.catalogId || work.id : work.id,
        mediaId: mode === "audio" ? work.id : undefined,
        novelName: work.title,
        authorName: work.author,
        novelCover: work.cover || "",
        lastUpdateTime: work.lastUpdateTime || "",
        bookshelfType: mode === "novel" ? "novel" : mode,
        sourcePath: work.onlinePath,
      },
      mode === "novel" ? "text" : mode,
    );
  }

  function backFromOnlineReading() {
    active.value = onlineReturnView.value;
  }

  function navigate(view: ViewName) {
    if (remoteDetailSessionActive && view !== "remoteDetail")
      clearRemoteDetailSession();
    if (
      view === "reader" ||
      view === "comicReader" ||
      view === "audioPlayer"
    ) {
      readerSource.value = "local";
    }
    active.value = view;
    if (view === "bookshelf") void bookshelf.loadBookshelf();
    if (view === "library") {
      void library.ensureLibraryAssetAccess().then((granted) => {
        if (granted) void library.refreshLibrary();
      });
    }
  }

  function continueDownload(mode: "novel" | "audio" | "comic") {
    if (mode === "audio" && !requireAudioAccess()) return;
    const book = library.localBook.value;
    const work = book?.[mode];
    if (!work) return;
    void chapterPicker.openChapterPicker(
      {
        novelId: mode === "audio" ? work.catalogId || work.id : work.id,
        mediaId: mode === "audio" ? work.id : undefined,
        novelName: work.title,
        authorName: work.author,
        novelCover: work.cover || "",
        lastUpdateTime: "",
      },
      mode === "novel" ? "text" : mode,
    );
  }

  function openChapterPicker(novel: Novel, mode: ChapterMode) {
    if (mode === "audio" && !requireAudioAccess()) return;
    return chapterPicker.openChapterPicker(novel, mode);
  }

  function formatDate(value?: string) {
    if (!value) return "未知时间";
    const date = new Date(value);
    return Number.isNaN(date.getTime())
      ? "未知时间"
      : new Intl.DateTimeFormat("zh-CN", {
          month: "short",
          day: "numeric",
        }).format(date);
  }

  function handleBackNavigation() {
    if (chapterPicker.chapterModalOpen.value) {
      chapterPicker.chapterModalOpen.value = false;
      return;
    }
    if (queueOpen.value) {
      queueOpen.value = false;
      return;
    }
    if (auth.credentialsOpen.value) {
      auth.credentialsOpen.value = false;
      return;
    }
    if (auth.accountOpen.value) {
      auth.accountOpen.value = false;
      return;
    }
    if (requestPolicy.requestPolicyOpen.value) {
      requestPolicy.requestPolicyOpen.value = false;
      return;
    }
    if (library.confirmBook.value) {
      library.confirmBook.value = undefined;
      return;
    }
    if (
      active.value === "reader" ||
      active.value === "comicReader" ||
      active.value === "audioPlayer"
    ) {
      if (readerSource.value === "online") backFromOnlineReading();
      else active.value = "libraryDetail";
      return;
    }
    if (active.value === "libraryDetail")
      active.value = library.backFromLibraryDetail();
    if (active.value === "remoteDetail") closeRemoteBookDetail();
  }

  function blockCopy(event: ClipboardEvent) {
    event.preventDefault();
  }

  function refreshLibraryAfterReturningToApp() {
    if (document.visibilityState === "visible" && active.value === "library")
      void library.refreshLibrary();
  }

  onMounted(() => {
    void onBackButtonPress(() => handleBackNavigation())
      .then((unlisten) => {
        if (disposed) void unlisten.unregister();
        else stopBackListener = unlisten;
      })
      .catch(() => {
        // The Android app plugin is unavailable in browser and desktop builds.
      });
    void auth.refreshAuthStatus().then(() => auth.loadAccountProfile());
    document.addEventListener("copy", blockCopy);
    document.addEventListener("visibilitychange", refreshLibraryAfterReturningToApp);
  });
  onBeforeUnmount(() => {
    disposed = true;
    void stopBackListener?.unregister();
    document.removeEventListener("copy", blockCopy);
    document.removeEventListener(
      "visibilitychange",
      refreshLibraryAfterReturningToApp,
    );
  });

  return {
    active,
    readerSource,
    queueOpen,
    ...search,
    ...jobs,
    ...library,
    ...bookshelf,
    ...chapterPicker,
    ...auth,
    ...requestPolicy,
    remoteBook,
    remoteBookLoading,
    remoteMedia,
    remoteMediaDetecting,
    remoteDetailReturnView,
    onlineReturnView,
    openRemoteBookDetail,
    closeRemoteBookDetail,
    selectRemoteMedia,
    startOnlineReading,
    startOnlineFromLibrary,
    backFromOnlineReading,
    continueDownload,
    openChapterPicker,
    requireAudioAccess,
    navigate,
    formatDate,
    ...online,
  };
}
