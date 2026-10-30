import { invoke } from "@tauri-apps/api/core";
import { ref, type Ref } from "vue";
import type { AuthStatus, Chapter, ChapterMode, ChapterVolume, Job, Novel } from "../types";
import { nativeErrorMessage, type Notify } from "./deskShared";

type UseChapterPickerOptions = {
  auth: Ref<AuthStatus>;
  notify: Notify;
  addJob: (job: Job) => void;
  refreshJobs: () => Promise<void>;
  loadCatalog?: (
    novel: Novel,
    mode: ChapterMode,
  ) => Promise<{ volumes?: ChapterVolume[]; chapters?: Chapter[] }>;
  loadDetail?: (
    novel: Novel,
    kind: "text" | "comic",
  ) => Promise<Partial<Novel> | undefined>;
};

export function useChapterPicker({
  auth,
  notify,
  addJob,
  refreshJobs,
  loadCatalog,
  loadDetail,
}: UseChapterPickerOptions) {
  const chapterModalOpen = ref(false);
  const chapterLoading = ref(false);
  const chapterMode = ref<ChapterMode>("text");
  const chapterNovel = ref<Novel>();
  const chapterVolumes = ref<ChapterVolume[]>([]);
  const audioChapters = ref<Chapter[]>([]);
  const comicChapters = ref<Chapter[]>([]);
  const selectedChapterIds = ref<number[]>([]);

  async function loadCatalogFor(novel: Novel, mode: ChapterMode) {
    if (loadCatalog) return loadCatalog(novel, mode);
    if (mode === "text") {
      return {
        volumes: await invoke<ChapterVolume[]>("get_chapter_volumes", {
          novelId: novel.novelId,
        }),
      };
    }
    const catalog = await invoke<{ chapters: Chapter[] }>(
      mode === "audio" ? "get_audio_chapters" : "get_comic_chapters",
      mode === "audio"
        ? {
            novelId: novel.novelId,
            albumId: novel.mediaId,
            titleHint: novel.novelName,
          }
        : {
            comicId: novel.novelId,
            sourcePath: novel.sourcePath,
            titleHint: novel.novelName,
          },
    );
    return { chapters: catalog.chapters };
  }

  async function loadDetailFor(novel: Novel, kind: "text" | "comic") {
    if (loadDetail) return loadDetail(novel, kind);
    return kind === "comic"
      ? invoke<Partial<Novel>>("get_comic_details", {
          comicId: novel.novelId,
        })
      : invoke<Partial<Novel>>("get_novel_details", {
          novelId: novel.novelId,
        });
  }

  async function openChapterPicker(novel: Novel, mode: ChapterMode) {
    chapterNovel.value = novel;
    chapterMode.value = mode;
    chapterModalOpen.value = true;
    chapterLoading.value = true;
    chapterVolumes.value = [];
    audioChapters.value = [];
    comicChapters.value = [];
    selectedChapterIds.value = [];
    try {
      if (mode === "text") {
        const details = await loadDetailFor(novel, "text").catch(() => ({}));
        chapterNovel.value = {
          ...novel,
          ...(details || {}),
        };
        chapterVolumes.value =
          (await loadCatalogFor(novel, "text")).volumes || [];
        if (auth.value.appAuthenticated) {
          const audio = await loadCatalogFor(novel, "audio").catch(
            () => undefined,
          );
          audioChapters.value = audio?.chapters || [];
        }
      } else if (mode === "audio") {
        audioChapters.value =
          (await loadCatalogFor(novel, "audio")).chapters || [];
      } else {
        const details = await loadDetailFor(novel, "comic").catch(
          () => undefined,
        );
        if (details) {
          chapterNovel.value = { ...novel, ...details };
        }
        comicChapters.value =
          (await loadCatalogFor(novel, "comic")).chapters || [];
      }
      selectDownloadableChapters();
    } catch (error) {
      chapterModalOpen.value = false;
      notify(nativeErrorMessage(error, "读取章节目录失败"), "error");
    } finally {
      chapterLoading.value = false;
    }
  }

  function allChapterIds(): number[] {
    return chapterMode.value === "text"
      ? chapterVolumes.value.flatMap((volume) =>
          volume.chapters.map((chapter) => chapter.chapId),
        )
      : chapterMode.value === "audio"
        ? audioChapters.value.map((chapter) => chapter.id)
        : comicChapters.value.map((chapter) => chapter.id);
  }

  function isDownloadable(id: number, mode = chapterMode.value) {
    const chapter = mode === "text"
      ? chapterVolumes.value
          .flatMap((volume) => volume.chapters)
          .find((item) => item.chapId === id)
      : mode === "audio"
        ? audioChapters.value.find((item) => item.id === id)
        : comicChapters.value.find((item) => item.id === id);
    if (!chapter || chapter.downloaded) return false;
    // Text and comic directory sales markers do not prove account entitlement.
    // The native downloader validates access only when it requests the resource.
    return mode === "text" || (mode === "comic" && "isUnlocked" in chapter && chapter.isUnlocked !== false) || !chapter.isVip ||
      ("isUnlocked" in chapter && chapter.isUnlocked === true);
  }

  function selectDownloadableChapters() {
    selectedChapterIds.value = allChapterIds().filter((id) => isDownloadable(id));
  }

  function toggleAllChapters() {
    const selectable = allChapterIds().filter((id) => isDownloadable(id));
    selectedChapterIds.value =
      selectedChapterIds.value.length === selectable.length ? [] : selectable;
  }

  function switchChapterMode(mode: ChapterMode) {
    if (mode === "text" && !chapterVolumes.value.length) return;
    if (mode === "audio" && !audioChapters.value.length) return;
    if (mode === "comic" && !comicChapters.value.length) return;
    chapterMode.value = mode;
    selectedChapterIds.value = allChapterIds().filter((id) => isDownloadable(id));
  }

  async function confirmChapterDownload() {
    const novel = chapterNovel.value;
    if (!novel || !selectedChapterIds.value.length)
      return notify("请至少选择一章");
    try {
      const job = chapterMode.value === "text"
        ? await invoke<Job>("create_text_download", {
            novelId: novel.novelId,
            title: novel.novelName,
            chapterIds: selectedChapterIds.value,
          })
        : chapterMode.value === "audio"
          ? await invoke<Job>("create_audio_download", {
              novelId: novel.novelId,
              albumId: novel.mediaId,
              title: novel.novelName,
              chapterIds: selectedChapterIds.value,
            })
          : await invoke<Job>("create_comic_download", {
              comicId: novel.novelId,
              sourcePath: novel.sourcePath,
              title: novel.novelName,
              chapterIds: selectedChapterIds.value,
            });
      addJob(job);
      chapterModalOpen.value = false;
      notify(`已将《${novel.novelName}》加入下载队列`);
      void refreshJobs();
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
