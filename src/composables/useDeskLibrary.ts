import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { onMounted, ref, type Ref } from "vue";
import type {
  Book,
  LocalBookDetail,
  LocalChapterContent,
  ViewName,
} from "../types";
import { localAssetSource, type Notify } from "./deskShared";

type ExportFormat = "epub" | "markdown" | "txt" | "audio";
type UseDeskLibraryOptions = {
  active: Ref<ViewName>;
  libraryDetailReturnView: Ref<ViewName>;
  notify: Notify;
};

export function useDeskLibrary({
  active,
  libraryDetailReturnView,
  notify,
}: UseDeskLibraryOptions) {
  const library = ref<Book[]>([]);
  const libraryManaging = ref(false);
  const confirmBook = ref<Book>();
  const localBook = ref<LocalBookDetail>();
  const localChapter = ref<LocalChapterContent>();
  const localAudioTrackIndex = ref(0);
  const exportingFormat = ref<ExportFormat>();

  async function refreshLibrary() {
    try {
      const books = await invoke<Book[]>("list_local_library");
      library.value = books.map((book) => ({
        ...book,
        cover: localAssetSource(book.cover),
      }));
    } catch {
      /* No local library exists yet. */
    }
  }

  function deleteBook(book: Book) {
    if (libraryManaging.value) confirmBook.value = book;
  }

  async function confirmDeleteBook() {
    const book = confirmBook.value;
    if (!book) return;
    try {
      await invoke<void>("delete_local_book", { name: book.name });
      library.value = library.value.filter((item) => item.name !== book.name);
      notify(`已删除《${book.name}》本地内容`);
    } catch (error) {
      notify(error instanceof Error ? error.message : "删除本地内容失败");
    }
    confirmBook.value = undefined;
  }

  async function openLibraryBook(book: Book) {
    try {
      const bookDetail = await invoke<LocalBookDetail>("get_local_book", {
        name: book.name,
      });
      bookDetail.cover = localAssetSource(bookDetail.cover);
      bookDetail.epubHref = localAssetSource(bookDetail.epubHref);
      bookDetail.audioTracks = bookDetail.audioTracks.map((track) => ({
        ...track,
        href: localAssetSource(track.href) || "",
      }));
      localBook.value = bookDetail;
      libraryDetailReturnView.value =
        active.value === "libraryDetail" ? "library" : active.value;
      active.value = "libraryDetail";
    } catch (error) {
      notify(error instanceof Error ? error.message : "无法读取本地书籍详情");
    }
  }

  async function openLocalChapter(chapterId: number) {
    const book = localBook.value;
    if (!book) return;
    try {
      localChapter.value = await invoke<LocalChapterContent>(
        "get_local_chapter",
        { name: book.name, chapterId },
      );
      active.value = "reader";
    } catch (error) {
      notify(error instanceof Error ? error.message : "无法读取本地章节");
    }
  }

  function openLocalAudioPlayer(trackIndex = 0) {
    if (!localBook.value?.audioTracks.length) {
      notify("本地没有可播放的有声章节");
      return;
    }
    localAudioTrackIndex.value = Math.min(
      Math.max(0, trackIndex),
      localBook.value.audioTracks.length - 1,
    );
    active.value = "audioPlayer";
  }

  async function readOnline() {
    const novelId = localBook.value?.novelId;
    if (!novelId) return;
    await openUrl(`https://book.sfacg.com/Novel/${novelId}/`);
  }

  async function exportBook(format: ExportFormat) {
    const book = localBook.value;
    if (!book) return;
    exportingFormat.value = format;
    try {
      const extension = format === "epub" ? "epub" : format === "txt" ? "txt" : "zip";
      const suffix =
        format === "markdown" ? "-Markdown" : format === "audio" ? "-有声" : "";
      const selectedPath = await save({
        title: "选择导出位置",
        defaultPath: `${book.name}${suffix}.${extension}`,
        filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
      });
      if (!selectedPath) return;
      const outputPath = selectedPath.toLowerCase().endsWith(`.${extension}`)
        ? selectedPath
        : `${selectedPath}.${extension}`;
      const result = await invoke<{ href: string; fileName: string }>(
        "export_local_book",
        { name: book.name, format, outputPath },
      );
      notify(`已导出到 ${result.href}`);
    } catch (error) {
      notify(error instanceof Error ? error.message : "导出失败");
    } finally {
      exportingFormat.value = undefined;
    }
  }

  function backFromLibraryDetail() {
    const target = libraryDetailReturnView.value;
    active.value = target === "libraryDetail" ? "library" : target;
  }

  onMounted(() => {
    void refreshLibrary();
  });

  return {
    library,
    libraryManaging,
    confirmBook,
    localBook,
    localChapter,
    localAudioTrackIndex,
    exportingFormat,
    refreshLibrary,
    deleteBook,
    confirmDeleteBook,
    openLibraryBook,
    openLocalChapter,
    openLocalAudioPlayer,
    readOnline,
    exportBook,
    backFromLibraryDetail,
  };
}
