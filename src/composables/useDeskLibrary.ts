import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { onMounted, ref, type Ref } from "vue";
import type {
  Book,
  LocalBookDetail,
  LocalChapterContent,
  LocalComicChapter,
  ViewName,
} from "../types";
import { localAssetSource, type Notify } from "./deskShared";

type ExportFormat = "epub" | "markdown" | "txt" | "audio" | "comic";
type UseDeskLibraryOptions = {
  libraryDetailReturnView: Ref<ViewName>;
  notify: Notify;
};

export function useDeskLibrary({
  libraryDetailReturnView,
  notify,
}: UseDeskLibraryOptions) {
  const library = ref<Book[]>([]);
  const libraryManaging = ref(false);
  const confirmBook = ref<Book>();
  const localBook = ref<LocalBookDetail>();
  const localChapter = ref<LocalChapterContent>();
  const localComicChapter = ref<LocalComicChapter>();
  const localAudioTrackIndex = ref(0);
  const exportingFormat = ref<ExportFormat>();
  let libraryAssetVersion = 0;

  async function refreshLibrary() {
    try {
      const books = await invoke<Book[]>("list_local_library");
      libraryAssetVersion += 1;
      library.value = books.map((book) => ({
        ...book,
        cover: localAssetSource(book.cover, libraryAssetVersion),
      }));
    } catch {
      /* No local library exists yet. */
    }
  }

  /**
   * Requests Android's all-files access only after the user enters the local
   * library. The public library directory cannot be read by the WebView asset
   * protocol without this permission.
   */
  async function ensureLibraryAssetAccess() {
    try {
      const granted = await invoke<boolean>("ensure_external_storage_access");
      if (!granted)
        notify("请在系统设置中允许管理所有文件，然后返回本地书库");
      return granted;
    } catch {
      // Browser previews do not expose the mobile command.
      return true;
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

  async function loadLocalBook(name: string) {
    try {
      const bookDetail = await invoke<LocalBookDetail>("get_local_book", {
        name,
      });
      for (const media of [
        bookDetail.novel,
        bookDetail.audio,
        bookDetail.comic,
      ]) {
        if (media) media.cover = localAssetSource(media.cover);
      }
      bookDetail.epubHref = localAssetSource(bookDetail.epubHref);
      bookDetail.audioTracks = bookDetail.audioTracks.map((track) => ({
        ...track,
        href: localAssetSource(track.href) || "",
      }));
      bookDetail.comicChapters = (bookDetail.comicChapters || []).map(
        (chapter) => ({
          ...chapter,
          cover: localAssetSource(chapter.cover),
        }),
      );
      localBook.value = bookDetail;
      return bookDetail;
    } catch (error) {
      notify(error instanceof Error ? error.message : "无法读取本地书籍详情");
      return undefined;
    }
  }

  async function openLocalComicChapter(chapterId: number) {
    const book = localBook.value;
    if (!book) return;
    try {
      const chapter = await invoke<LocalComicChapter>(
        "get_local_comic_chapter",
        {
          name: book.name,
          chapterId,
        },
      );
      chapter.pages = chapter.pages.map((page) => localAssetSource(page) || "");
      localChapter.value = undefined;
      localComicChapter.value = chapter;
      return chapter;
    } catch (error) {
      notify(error instanceof Error ? error.message : "无法读取本地漫画章节");
    }
  }

  async function openLocalChapter(chapterId: number) {
    const book = localBook.value;
    if (!book) return;
    try {
      const chapter = await invoke<LocalChapterContent>(
        "get_local_chapter",
        { name: book.name, chapterId },
      );
      localComicChapter.value = undefined;
      localChapter.value = chapter;
      return chapter;
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
  }

  async function readOnline(mode: "novel" | "audio" | "comic") {
    const book = localBook.value;
    const work = book?.[mode];
    if (!work) return;
    const url =
      mode === "comic"
        ? work.onlinePath
          ? `https://manhua.sfacg.com/mh/${work.onlinePath}/`
          : `https://manhua.sfacg.com/mh/${work.id}/`
        : mode === "audio"
          ? `https://i.sfacg.com/consume/book/?nid=${work.catalogId || work.id}`
          : `https://book.sfacg.com/Novel/${work.id}/`;
    await openUrl(url);
  }

  async function exportBook(format: ExportFormat) {
    const book = localBook.value;
    if (!book) return;
    exportingFormat.value = format;
    try {
      const extension =
        format === "epub" ? "epub" : format === "txt" ? "txt" : "zip";
      const suffix =
        format === "markdown"
          ? "-Markdown"
          : format === "audio"
            ? "-有声"
            : format === "comic"
              ? "-漫画"
              : "";
      const selectedPath = await save({
        title: "选择导出位置",
        defaultPath: `${book.name}${suffix}.${extension}`,
        filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
      });
      if (!selectedPath) return;
      const isDocumentUri = selectedPath.startsWith("content://");
      const outputPath =
        isDocumentUri || selectedPath.toLowerCase().endsWith(`.${extension}`)
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
    return target === "libraryDetail" ? "library" : target;
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
    localComicChapter,
    localAudioTrackIndex,
    exportingFormat,
    refreshLibrary,
    ensureLibraryAssetAccess,
    loadLocalBook,
    deleteBook,
    confirmDeleteBook,
    openLocalChapter,
    openLocalComicChapter,
    openLocalAudioPlayer,
    readOnline,
    exportBook,
    backFromLibraryDetail,
  };
}
