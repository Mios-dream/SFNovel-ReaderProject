import { onBackButtonPress } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { onBeforeUnmount, onMounted, ref } from "vue";
import type { Novel, ViewName } from "../types";
import { useChapterPicker } from "./useChapterPicker";
import { useDeskAuth } from "./useDeskAuth";
import { useDeskBookshelf } from "./useDeskBookshelf";
import { useDeskJobs } from "./useDeskJobs";
import { useDeskLibrary } from "./useDeskLibrary";
import { useNovelSearch } from "./useNovelSearch";
import { useRequestPolicy } from "./useRequestPolicy";

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
  const libraryDetailReturnView = ref<ViewName>("library");
  const queueOpen = ref(false);
  const toast = ref("");
  const remoteBook = ref<Novel>();
  const remoteBookLoading = ref(false);
  const remoteMedia = ref<RemoteMedia>({});
  const remoteMediaDetecting = ref(false);
  let toastTimer: number | undefined;
  let stopBackListener: { unregister: () => Promise<void> } | undefined;
  let disposed = false;
  let remoteBookRequest = 0;

  function notify(message: string) {
    toast.value = message;
    window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => {
      toast.value = "";
    }, 2800);
  }

  const auth = useDeskAuth(notify);
  const requestPolicy = useRequestPolicy(notify);
  const library = useDeskLibrary({
    libraryDetailReturnView,
    notify,
  });
  const jobs = useDeskJobs({
    notify,
    refreshLibrary: library.refreshLibrary,
  });
  const search = useNovelSearch({
    refreshAuthStatus: auth.refreshAuthStatus,
    notify,
  });
  const bookshelf = useDeskBookshelf({
    auth: auth.auth,
    refreshAuthStatus: auth.refreshAuthStatus,
    requestCredentials: () => {
      auth.credentialsOpen.value = true;
    },
    notify,
  });
  const chapterPicker = useChapterPicker({
    auth: auth.auth,
    notify,
    addJob: jobs.upsertJob,
    refreshJobs: jobs.refreshJobs,
  });

  async function openRemoteBookDetail(novel: Novel, preserveMedia = false) {
    const request = ++remoteBookRequest;
    remoteBook.value = novel;
    remoteBookLoading.value = true;
    active.value = "remoteDetail";
    if (!preserveMedia) {
      remoteMedia.value = { [remoteMediaType(novel)]: novel };
      remoteMediaDetecting.value = true;
    } else {
      remoteMediaDetecting.value = false;
    }
    try {
      const details = novel.bookshelfType === "comic"
        ? await invoke<Partial<Novel>>("get_comic_details", {
            comicId: novel.novelId,
          })
        : novel.bookshelfType === "audio"
          ? novel.mediaId
            ? await invoke<Partial<Novel>>("get_audio_details", {
                albumId: novel.mediaId,
                novelId: novel.novelId,
              })
            : undefined
          : await invoke<Partial<Novel>>("get_novel_details", {
              novelId: novel.novelId,
            });
      if (details && request === remoteBookRequest) {
        remoteBook.value = { ...novel, ...details };
        remoteMedia.value = {
          ...remoteMedia.value,
          [remoteMediaType(novel)]: remoteBook.value,
        };
      }
    } catch (error) {
      if (request === remoteBookRequest)
        notify(error instanceof Error ? error.message : "读取作品详情失败");
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

  function navigate(view: ViewName) {
    active.value = view;
    if (view === "bookshelf") void bookshelf.loadBookshelf();
    if (view === "library") {
      void library.ensureLibraryAssetAccess().then((granted) => {
        if (granted) void library.refreshLibrary();
      });
    }
  }

  function continueDownload(mode: "novel" | "audio" | "comic") {
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
      active.value = "libraryDetail";
      return;
    }
    if (active.value === "libraryDetail")
      active.value = library.backFromLibraryDetail();
    if (active.value === "remoteDetail") active.value = "bookshelf";
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
    window.clearTimeout(toastTimer);
    document.removeEventListener("copy", blockCopy);
    document.removeEventListener(
      "visibilitychange",
      refreshLibraryAfterReturningToApp,
    );
  });

  return {
    active,
    queueOpen,
    toast,
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
    openRemoteBookDetail,
    selectRemoteMedia,
    continueDownload,
    navigate,
    formatDate,
  };
}
