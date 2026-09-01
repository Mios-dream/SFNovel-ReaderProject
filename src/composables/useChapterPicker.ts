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
  const chapterNovel = ref<Novel>();
  const chapterVolumes = ref<ChapterVolume[]>([]);
  const audioChapters = ref<Chapter[]>([]);
  const selectedChapterIds = ref<number[]>([]);

  async function openChapterPicker(novel: Novel, mode: ChapterMode) {
    if (novel.bookshelfType === "comic") {
      notify("当前暂不支持漫画章节下载");
      return;
    }
    chapterNovel.value = novel;
    chapterMode.value = mode;
    chapterModalOpen.value = true;
    chapterLoading.value = true;
    chapterVolumes.value = [];
    audioChapters.value = [];
    chapterHasAudio.value = false;
    selectedChapterIds.value = [];
    try {
      const [info, volumes, audio] = await Promise.all([
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
      ]);
      chapterNovel.value = { ...novel, ...info };
      chapterVolumes.value = volumes;
      audioChapters.value = audio.chapters;
      chapterHasAudio.value = audio.chapters.length > 0;
      if (mode === "audio" && !chapterHasAudio.value)
        chapterMode.value = "text";
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
      : audioChapters.value.map((chapter) => chapter.id);
  }

  function isDownloadable(id: number, mode = chapterMode.value) {
    const chapter = mode === "text"
      ? chapterVolumes.value
          .flatMap((volume) => volume.chapters)
          .find((item) => item.chapId === id)
      : audioChapters.value.find((item) => item.id === id);
    return chapter && !chapter.downloaded && (!chapter.isVip || chapter.isUnlocked);
  }

  function selectDownloadableChapters() {
    selectedChapterIds.value = allChapterIds().filter((id) => isDownloadable(id));
  }

  function changeChapterMode(mode: ChapterMode) {
    if (mode === "audio" && !chapterHasAudio.value) return;
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
        : await invoke<Job>("create_audio_download", {
            novelId: novel.novelId,
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
    chapterNovel,
    chapterVolumes,
    audioChapters,
    selectedChapterIds,
    openChapterPicker,
    changeChapterMode,
    toggleAllChapters,
    confirmChapterDownload,
  };
}
