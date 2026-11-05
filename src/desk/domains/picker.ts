import { ref, type Ref } from "vue";
import type {
  AuthStatus,
  Chapter,
  ChapterVolume,
  MediaType,
  Novel,
} from "../../types";
import { nativeErrorMessage } from "../../utils/native";
import type { Notify } from "../../composables/useToast";

type PickerDeskOptions = {
  auth: Ref<AuthStatus>;
  notify: Notify;
  /** 目录由在线阅读域提供：弹窗与阅读页必须看到同一份目录。 */
  loadCatalog: (
    novel: Novel,
    mode: MediaType,
  ) => Promise<{ volumes?: ChapterVolume[]; chapters?: Chapter[] }>;
  /** 详情由远端详情域提供：复用会话缓存，避免重复请求。 */
  loadDetail: (
    novel: Novel,
    kind: "novel" | "comic",
  ) => Promise<Partial<Novel> | undefined>;
  /** 创建下载任务；入队、提示与队列刷新都由下载域完成。 */
  enqueueDownload: (
    mode: MediaType,
    novel: Novel,
    chapterIds: number[],
  ) => Promise<void>;
};

/**
 * 章节选择弹窗。
 *
 * 只负责「选哪些章」与弹窗开关；目录来自在线阅读域、任务创建来自下载域，因此
 * 这里没有任何上游命令调用，也不维护自己的目录缓存。
 */
export function createPickerDesk({
  auth,
  notify,
  loadCatalog,
  loadDetail,
  enqueueDownload,
}: PickerDeskOptions) {
  const chapterModalOpen = ref(false);
  const chapterLoading = ref(false);
  const chapterMode = ref<MediaType>("novel");
  const chapterNovel = ref<Novel>();
  const chapterVolumes = ref<ChapterVolume[]>([]);
  const audioChapters = ref<Chapter[]>([]);
  const comicChapters = ref<Chapter[]>([]);
  const selectedChapterIds = ref<number[]>([]);

  async function openChapterPicker(novel: Novel, mode: MediaType) {
    chapterNovel.value = novel;
    chapterMode.value = mode;
    chapterModalOpen.value = true;
    chapterLoading.value = true;
    chapterVolumes.value = [];
    audioChapters.value = [];
    comicChapters.value = [];
    selectedChapterIds.value = [];
    try {
      if (mode === "novel") {
        const details = await loadDetail(novel, "novel").catch(() => ({}));
        chapterNovel.value = { ...novel, ...(details || {}) };
        chapterVolumes.value = (await loadCatalog(novel, "novel")).volumes || [];
        if (auth.value.appAuthenticated) {
          const audio = await loadCatalog(novel, "audio").catch(
            () => undefined,
          );
          audioChapters.value = audio?.chapters || [];
        }
      } else if (mode === "audio") {
        audioChapters.value = (await loadCatalog(novel, "audio")).chapters || [];
      } else {
        const details = await loadDetail(novel, "comic").catch(
          () => undefined,
        );
        if (details) {
          chapterNovel.value = { ...novel, ...details };
        }
        comicChapters.value = (await loadCatalog(novel, "comic")).chapters || [];
      }
      selectDownloadableChapters();
    } catch (error) {
      chapterModalOpen.value = false;
      notify(nativeErrorMessage(error, "读取章节目录失败"), "error");
    } finally {
      chapterLoading.value = false;
    }
  }

  function downloadableChapterIds(mode = chapterMode.value): number[] {
    if (mode === "novel")
      return chapterVolumes.value.flatMap((volume) =>
        volume.chapters
          .filter(
            (chapter) =>
              !chapter.downloaded && chapter.accessState === "available",
          )
          .map((chapter) => chapter.chapId),
      );
    if (mode === "audio")
      return audioChapters.value
        .filter(
          (chapter) =>
            !chapter.downloaded &&
            (!chapter.isVip || chapter.isUnlocked === true),
        )
        .map((chapter) => chapter.id);
    return comicChapters.value
      .filter((chapter) => !chapter.downloaded && chapter.isUnlocked !== false)
      .map((chapter) => chapter.id);
  }

  function selectDownloadableChapters() {
    selectedChapterIds.value = downloadableChapterIds();
  }

  function toggleAllChapters() {
    const selectable = downloadableChapterIds();
    const selected = new Set(selectedChapterIds.value);
    selectedChapterIds.value =
      selectable.length > 0 && selectable.every((id) => selected.has(id))
        ? []
        : selectable;
  }

  function switchChapterMode(mode: MediaType) {
    if (mode === "novel" && !chapterVolumes.value.length) return;
    if (mode === "audio" && !audioChapters.value.length) return;
    if (mode === "comic" && !comicChapters.value.length) return;
    chapterMode.value = mode;
    selectedChapterIds.value = downloadableChapterIds(mode);
  }

  async function confirmChapterDownload() {
    const novel = chapterNovel.value;
    if (!novel || !selectedChapterIds.value.length)
      return notify("请至少选择一章");
    try {
      await enqueueDownload(chapterMode.value, novel, selectedChapterIds.value);
      chapterModalOpen.value = false;
    } catch (error) {
      notify(nativeErrorMessage(error, "创建下载任务失败"), "error");
    }
  }

  return {
    chapterModalOpen,
    chapterLoading,
    chapterMode,
    chapterNovel,
    chapterVolumes,
    audioChapters,
    comicChapters,
    selectedChapterIds,
    openChapterPicker,
    toggleAllChapters,
    confirmChapterDownload,
    switchChapterMode,
  };
}

/** 该域对外暴露的完整上下文；页面只应通过 `usePicker` 取用。 */
export type PickerDesk = ReturnType<typeof createPickerDesk>;
