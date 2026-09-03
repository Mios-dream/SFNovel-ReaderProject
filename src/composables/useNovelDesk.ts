import { onBackButtonPress } from "@tauri-apps/api/app";
import { onBeforeUnmount, onMounted, ref } from "vue";
import type { ViewName } from "../types";
import { useChapterPicker } from "./useChapterPicker";
import { useDeskAuth } from "./useDeskAuth";
import { useDeskBookshelf } from "./useDeskBookshelf";
import { useDeskJobs } from "./useDeskJobs";
import { useDeskLibrary } from "./useDeskLibrary";
import { useNovelSearch } from "./useNovelSearch";
import { useRequestPolicy } from "./useRequestPolicy";

/**
 * Composes the desktop's domain modules and owns cross-domain navigation.
 * Individual API calls and domain state deliberately live in their modules.
 */
export function useNovelDesk() {
  const active = ref<ViewName>("discover");
  const libraryDetailReturnView = ref<ViewName>("library");
  const queueOpen = ref(false);
  const toast = ref("");
  let toastTimer: number | undefined;
  let stopBackListener: { unregister: () => Promise<void> } | undefined;
  let disposed = false;

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

  function navigate(view: ViewName) {
    active.value = view;
    if (view === "bookshelf") void bookshelf.refreshBookshelf();
  }

  function continueDownload() {
    const book = library.localBook.value;
    if (!book?.novelId) return;
    void chapterPicker.openChapterPicker(
      {
        novelId: book.novelId,
        novelName: book.name,
        authorName: book.author,
        novelCover: book.cover || "",
        lastUpdateTime: "",
      },
      "text",
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
    if (active.value === "reader" || active.value === "audioPlayer") {
      active.value = "libraryDetail";
      return;
    }
    if (active.value === "libraryDetail")
      active.value = library.backFromLibraryDetail();
  }

  function blockCopy(event: ClipboardEvent) {
    event.preventDefault();
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
  });
  onBeforeUnmount(() => {
    disposed = true;
    void stopBackListener?.unregister();
    window.clearTimeout(toastTimer);
    document.removeEventListener("copy", blockCopy);
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
    continueDownload,
    navigate,
    formatDate,
  };
}
