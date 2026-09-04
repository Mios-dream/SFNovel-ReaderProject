import { invoke } from "@tauri-apps/api/core";
import { ref, type Ref } from "vue";
import type { AuthStatus, Chapter, ChapterMode, ChapterVolume, Job, Novel } from "../types";
import type { Notify } from "./deskShared";

type UseChapterPickerOptions = {
  auth: Ref<AuthStatus>;
  notify: Notify;
  addJob: (job: Job) => void;
  refreshJobs: () => Promise<void>;
};

export function useChapterPicker({
  auth,
  notify,
  addJob,
  refreshJobs,
}: UseChapterPickerOptions) {
  const chapterModalOpen = ref(false);
  const chapterLoading = ref(false);
  const chapterMode = ref<ChapterMode>("text");
  const chapterHasAudio = ref(false);
  const chapterHasComic = ref(false);
  const chapterNovel = ref<Novel>();
  const chapterVolumes = ref<ChapterVolume[]>([]);
  const audioChapters = ref<Chapter[]>([]);
  const comicChapters = ref<Chapter[]>([]);
  const selectedChapterIds = ref<number[]>([]);

  async function openChapterPicker(novel: Novel, mode: ChapterMode) {
    chapterNovel.value = novel;
    chapterMode.value = mode;
    chapterModalOpen.value = true;
    chapterLoading.value = true;
    chapterVolumes.value = [];
    audioChapters.value = [];
    comicChapters.value = [];
    chapterHasAudio.value = false;
    chapterHasComic.value = false;
    selectedChapterIds.value = [];
    try {
      const [info, volumes, audio, comic] = await Promise.all([
        invoke<Partial<Novel>>("get_novel_details", {
          novelId: novel.novelId,
        }).catch(() => ({})),
        mode === "text"
          ? invoke<ChapterVolume[]>("get_chapter_volumes", {
              novelId: novel.novelId,
            }).catch(() => [])
          : Promise.resolve([] as ChapterVolume[]),
        auth.value.authenticated
          ? invoke<{ chapters: Chapter[] }>("get_audio_chapters", {
              novelId: novel.novelId,
            }).catch(() => ({ chapters: [] as Chapter[] }))
          : Promise.resolve({ chapters: [] as Chapter[] }),
        novel.bookshelfType === "comic"
          ? invoke<{ chapters: Chapter[] }>("get_comic_chapters", {
              comicId: novel.novelId,
            }).catch(() => ({ chapters: [] as Chapter[] }))
          : Promise.resolve({ chapters: [] as Chapter[] }),
      ]);
      chapterNovel.value = { ...novel, ...info };
      chapterVolumes.value = volumes;
      audioChapters.value = audio.chapters;
      comicChapters.value = comic.chapters;
      chapterHasAudio.value = audio.chapters.length > 0;
      chapterHasComic.value = comic.chapters.length > 0;
      if (mode === "audio" && !chapterHasAudio.value)
        chapterMode.value = "text";
      if (mode === "comic" && !chapterHasComic.value)
        chapterModalOpen.value = false;
      selectDownloadableChapters();
    } catch (error) {
      chapterModalOpen.value = false;
      notify(error instanceof Error ? error.message : "读取章节目录失败");
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
    return chapter && !chapter.downloaded && (!chapter.isVip || chapter.isUnlocked);
  }

  function selectDownloadableChapters() {
    selectedChapterIds.value = allChapterIds().filter((id) => isDownloadable(id));
  }

  function changeChapterMode(mode: ChapterMode) {
    if (mode === "audio" && !chapterHasAudio.value) return;
    if (mode === "comic" && !chapterHasComic.value) return;
    chapterMode.value = mode;
    selectDownloadableChapters();
  }

  function toggleAllChapters() {
    const selectable = allChapterIds().filter((id) => isDownloadable(id));
    selectedChapterIds.value =
      selectedChapterIds.value.length === selectable.length ? [] : selectable;
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
              title: novel.novelName,
              chapterIds: selectedChapterIds.value,
            });
      addJob(job);
      chapterModalOpen.value = false;
      notify(`已将《${novel.novelName}》加入下载队列`);
      void refreshJobs();
    } catch (error) {
      notify(error instanceof Error ? error.message : "创建下载任务失败");
    }
  }

  return {
    chapterModalOpen,
    chapterLoading,
    chapterMode,
    chapterHasAudio,
    chapterHasComic,
    chapterNovel,
    chapterVolumes,
    audioChapters,
    comicChapters,
    selectedChapterIds,
    openChapterPicker,
    changeChapterMode,
    toggleAllChapters,
    confirmChapterDownload,
  };
}
