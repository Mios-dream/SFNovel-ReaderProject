import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { computed, ref } from "vue";
import type {
  Chapter,
  ChapterMode,
  ChapterVolume,
  Novel,
  OnlineAudioTrack,
  OnlineComicChapter,
  OnlineTextChapter,
} from "../types";
import { nativeErrorMessage, type Notify } from "./deskShared";

type UseOnlineReadingOptions = {
  notify: Notify;
};

type ChapterIdentity = { id: number };

/** 内存中保留的在线内容数量，文字章节可用缓存覆盖当前及前后各一章。 */
const ONLINE_CONTENT_CACHE_LIMIT = 5;

/**
 * 在线浏览的共享状态。
 *
 * 目录与章节内容都按需读取；页面关闭后仍保留当前作品，便于在详情页与阅读器
 * 之间返回。文字章节会保留在内存中，并在打开一章后预取前后相邻章节，减少
 * 连续阅读时的等待；漫画按页懒加载，有声通过原生媒体协议流式播放。
 */
export function useOnlineReading({ notify }: UseOnlineReadingOptions) {
  const onlineBook = ref<Novel>();
  const onlineMode = ref<ChapterMode>("text");
  const pendingChapterId = ref<number>();
  const textVolumes = ref<ChapterVolume[]>([]);
  const audioChapters = ref<Chapter[]>([]);
  const comicChapters = ref<Chapter[]>([]);
  const directoryLoading = ref(false);
  const directoryError = ref("");
  const chapterLoading = ref(false);
  const chapterError = ref("");
  const textChapter = ref<OnlineTextChapter>();
  const comicChapter = ref<OnlineComicChapter>();
  const audioTrack = ref<OnlineAudioTrack>();
  const audioTrackLoading = ref(false);
  let loadedDirectoryKey = "";
  let cacheGeneration = 0;
  let textOpenRequest = 0;
  let comicOpenRequest = 0;
  let audioOpenRequest = 0;

  const textChapterCache = new Map<number, OnlineTextChapter>();
  const comicChapterCache = new Map<number, OnlineComicChapter>();
  const audioTrackCache = new Map<number, OnlineAudioTrack>();
  const textChapterRequests = new Map<number, Promise<OnlineTextChapter>>();
  const comicChapterRequests = new Map<number, Promise<OnlineComicChapter>>();
  const audioTrackRequests = new Map<number, Promise<OnlineAudioTrack>>();
  let directoryRequest:
    | { key: string; promise: Promise<void> }
    | undefined;
  let directoryRequestId = 0;

  const textChapters = computed(() =>
    textVolumes.value.flatMap((volume) =>
      volume.chapters.map((chapter) => ({
        id: chapter.chapId,
        title: chapter.title,
        volume: volume.title,
        isVip: chapter.isVip,
      })),
    ),
  );

  function directoryKey(book: Novel, mode: ChapterMode) {
    return `${mode}:${book.novelId}:${book.mediaId || ""}:${book.sourcePath || ""}`;
  }

  function rememberChapter<Key, Value>(
    cache: Map<Key, Value>,
    key: Key,
    value: Value,
  ) {
    // 重新插入以维护最近使用顺序，并限制常驻内容数量。
    cache.delete(key);
    cache.set(key, value);
    while (cache.size > ONLINE_CONTENT_CACHE_LIMIT) {
      const oldest = cache.keys().next().value;
      if (oldest === undefined) break;
      cache.delete(oldest);
    }
  }

  function clearChapterCaches() {
    cacheGeneration += 1;
    textChapterCache.clear();
    comicChapterCache.clear();
    audioTrackCache.clear();
    textChapterRequests.clear();
    comicChapterRequests.clear();
    audioTrackRequests.clear();
  }

  function prepareOnlineReading(
    book: Novel,
    mode: ChapterMode,
    chapterId?: number,
  ) {
    const nextDirectoryKey = directoryKey(book, mode);
    if (
      !onlineBook.value ||
      directoryKey(onlineBook.value, onlineMode.value) !== nextDirectoryKey
    ) {
      loadedDirectoryKey = "";
      textVolumes.value = [];
      audioChapters.value = [];
      comicChapters.value = [];
      clearChapterCaches();
      directoryLoading.value = false;
      directoryRequest = undefined;
      textOpenRequest += 1;
      comicOpenRequest += 1;
      audioOpenRequest += 1;
    }
    onlineBook.value = book;
    onlineMode.value = mode;
    pendingChapterId.value = chapterId;
    directoryError.value = "";
    chapterError.value = "";
    textChapter.value = undefined;
    comicChapter.value = undefined;
    audioTrack.value = undefined;
  }

  /**
   * 在详情页提前准备目录，并复用阅读器所用的同一份共享缓存。
   * 同一作品的详情刷新不会清空已下载的章节内容。
   */
  async function primeOnlineDirectory(
    book: Novel,
    mode: ChapterMode,
    force = false,
  ) {
    const nextKey = directoryKey(book, mode);
    const currentKey = onlineBook.value
      ? directoryKey(onlineBook.value, onlineMode.value)
      : "";
    if (currentKey !== nextKey) {
      prepareOnlineReading(book, mode);
    } else {
      onlineBook.value = book;
      onlineMode.value = mode;
    }
    await loadDirectory(force);
  }

  async function loadDirectory(force = false) {
    const book = onlineBook.value;
    if (!book) throw new Error("未选择在线作品");
    const key = directoryKey(book, onlineMode.value);
    if (!force && loadedDirectoryKey === key) return;
    if (!force && directoryRequest?.key === key)
      return directoryRequest.promise;
    const generation = cacheGeneration;
    const requestId = ++directoryRequestId;
    directoryLoading.value = true;
    directoryError.value = "";
    const request = (async () => {
      try {
        if (onlineMode.value === "text") {
          const volumes = await invoke<ChapterVolume[]>(
            "get_chapter_volumes",
            { novelId: book.novelId },
          );
          if (generation !== cacheGeneration || requestId !== directoryRequestId)
            return;
          textVolumes.value = volumes;
        } else if (onlineMode.value === "audio") {
          const catalog = await invoke<{ chapters: Chapter[] }>(
            "get_audio_chapters",
            {
              novelId: book.novelId,
              albumId: book.mediaId,
              titleHint: book.novelName,
            },
          );
          if (generation !== cacheGeneration || requestId !== directoryRequestId)
            return;
          audioChapters.value = catalog.chapters;
        } else {
          const catalog = await invoke<{ chapters: Chapter[] }>(
            "get_comic_chapters",
            {
              comicId: book.novelId,
              sourcePath: book.sourcePath,
              titleHint: book.novelName,
            },
          );
          if (generation !== cacheGeneration || requestId !== directoryRequestId)
            return;
          comicChapters.value = catalog.chapters;
        }
        loadedDirectoryKey = key;
      } catch (error) {
        if (generation === cacheGeneration && requestId === directoryRequestId)
          directoryError.value = nativeErrorMessage(
            error,
            "读取在线目录失败",
          );
        throw error;
      } finally {
        if (requestId === directoryRequestId)
          directoryLoading.value = false;
      }
    })();
    directoryRequest = { key, promise: request };
    try {
      await request;
    } finally {
      if (directoryRequest?.promise === request) directoryRequest = undefined;
    }
  }

  function surroundingChapterIds(
    chapters: ChapterIdentity[],
    chapterId: number,
  ) {
    const index = chapters.findIndex((chapter) => chapter.id === chapterId);
    if (index < 0) return [];
    // 优先预取下一章，随后预取上一章；没有的一侧自动跳过。
    return [chapters[index + 1]?.id, chapters[index - 1]?.id].filter(
      (id): id is number => typeof id === "number",
    );
  }

  function fetchOnlineTextChapter(chapterId: number) {
    const cached = textChapterCache.get(chapterId);
    if (cached) {
      rememberChapter(textChapterCache, chapterId, cached);
      return Promise.resolve(cached);
    }
    const pending = textChapterRequests.get(chapterId);
    if (pending) return pending;
    const book = onlineBook.value;
    if (!book) return Promise.reject(new Error("未选择在线作品"));
    const generation = cacheGeneration;
    const request = invoke<OnlineTextChapter>("load_online_text_chapter", {
      novelId: book.novelId,
      chapterId,
    })
      .then((payload) => {
        const chapter = {
          ...payload,
          images: payload.images.map((path) => convertFileSrc(path)),
        };
        if (generation === cacheGeneration)
          rememberChapter(textChapterCache, chapterId, chapter);
        return chapter;
      })
      .finally(() => {
        if (textChapterRequests.get(chapterId) === request)
          textChapterRequests.delete(chapterId);
      });
    textChapterRequests.set(chapterId, request);
    return request;
  }

  async function prefetchOnlineTextNeighbors(chapterId: number) {
    const ids = surroundingChapterIds(textChapters.value, chapterId);
    await Promise.allSettled(ids.map(fetchOnlineTextChapter));
  }

  async function openOnlineTextChapter(chapterId: number) {
    const book = onlineBook.value;
    if (!book || chapterLoading.value) return;
    const generation = cacheGeneration;
    const request = ++textOpenRequest;
    chapterLoading.value = true;
    chapterError.value = "";
    try {
      await loadDirectory();
      const payload = await fetchOnlineTextChapter(chapterId);
      if (generation !== cacheGeneration || request !== textOpenRequest) return;
      textChapter.value = payload;
      pendingChapterId.value = chapterId;
      void prefetchOnlineTextNeighbors(chapterId);
    } catch (error) {
      if (generation === cacheGeneration && request === textOpenRequest) {
        chapterError.value = nativeErrorMessage(error, "读取在线章节失败");
        notify(chapterError.value, "error");
      }
    } finally {
      if (request === textOpenRequest) chapterLoading.value = false;
    }
  }

  /**
   * 读取在线文字章节但不改变当前阅读状态，供翻页模式预渲染相邻章节边界页。
   * 命中预取缓存时不会再次请求网络。
   * @returns 章节内容；读取失败时返回 undefined。
   */
  async function peekOnlineTextChapter(chapterId: number) {
    if (!onlineBook.value) return undefined;
    try {
      await loadDirectory();
      return await fetchOnlineTextChapter(chapterId);
    } catch {
      return undefined;
    }
  }

  function fetchOnlineComicChapter(chapterId: number) {
    const cached = comicChapterCache.get(chapterId);
    if (cached) {
      rememberChapter(comicChapterCache, chapterId, cached);
      return Promise.resolve(cached);
    }
    const pending = comicChapterRequests.get(chapterId);
    if (pending) return pending;
    const book = onlineBook.value;
    if (!book) return Promise.reject(new Error("未选择在线作品"));
    const chapter = comicChapters.value.find((item) => item.id === chapterId);
    const generation = cacheGeneration;
    const request = invoke<OnlineComicChapter>("load_online_comic_chapter", {
      comicId: book.novelId,
      chapterId,
      chapterTitle: chapter?.title || `章节 ${chapterId}`,
      sourcePath: book.sourcePath,
    })
      .then((payload) => {
        if (generation === cacheGeneration)
          rememberChapter(comicChapterCache, chapterId, payload);
        return payload;
      })
      .finally(() => {
        if (comicChapterRequests.get(chapterId) === request)
          comicChapterRequests.delete(chapterId);
      });
    comicChapterRequests.set(chapterId, request);
    return request;
  }

  async function openOnlineComicChapter(chapterId: number) {
    const book = onlineBook.value;
    if (!book || chapterLoading.value) return;
    const generation = cacheGeneration;
    const request = ++comicOpenRequest;
    chapterLoading.value = true;
    chapterError.value = "";
    try {
      await loadDirectory();
      const payload = await fetchOnlineComicChapter(chapterId);
      if (generation !== cacheGeneration || request !== comicOpenRequest) return;
      comicChapter.value = payload;
      pendingChapterId.value = chapterId;
    } catch (error) {
      if (generation === cacheGeneration && request === comicOpenRequest) {
        chapterError.value = nativeErrorMessage(error, "读取在线漫画失败");
        notify(chapterError.value, "error");
      }
    } finally {
      if (request === comicOpenRequest) chapterLoading.value = false;
    }
  }

  function fetchOnlineAudioTrack(chapterId: number) {
    const cached = audioTrackCache.get(chapterId);
    if (cached) {
      rememberChapter(audioTrackCache, chapterId, cached);
      return Promise.resolve(cached);
    }
    const pending = audioTrackRequests.get(chapterId);
    if (pending) return pending;
    const book = onlineBook.value;
    if (!book) return Promise.reject(new Error("未选择在线作品"));
    const generation = cacheGeneration;
    const request = invoke<OnlineAudioTrack>("load_online_audio_track", {
      novelId: book.novelId,
      albumId: book.mediaId,
      titleHint: book.novelName,
      chapterId,
    })
      .then((payload) => {
        if (generation === cacheGeneration)
          rememberChapter(audioTrackCache, chapterId, payload);
        return payload;
      })
      .finally(() => {
        if (audioTrackRequests.get(chapterId) === request)
          audioTrackRequests.delete(chapterId);
      });
    audioTrackRequests.set(chapterId, request);
    return request;
  }

  async function loadOnlineAudioTrack(chapterId: number, play = false) {
    const book = onlineBook.value;
    if (!book || audioTrackLoading.value) return undefined;
    if (audioTrack.value?.id === chapterId) {
      if (play) pendingChapterId.value = chapterId;
      return audioTrack.value;
    }
    const generation = cacheGeneration;
    const request = ++audioOpenRequest;
    audioTrackLoading.value = true;
    chapterError.value = "";
    try {
      const payload = await fetchOnlineAudioTrack(chapterId);
      if (generation !== cacheGeneration || request !== audioOpenRequest)
        return undefined;
      audioTrack.value = payload;
      pendingChapterId.value = chapterId;
      return payload;
    } catch (error) {
      if (generation === cacheGeneration && request === audioOpenRequest) {
        chapterError.value = nativeErrorMessage(error, "读取在线音频失败");
        notify(chapterError.value, "error");
      }
      return undefined;
    } finally {
      if (request === audioOpenRequest) audioTrackLoading.value = false;
    }
  }

  return {
    onlineBook,
    pendingChapterId,
    textVolumes,
    audioChapters,
    comicChapters,
    textChapters,
    directoryLoading,
    directoryError,
    chapterLoading,
    chapterError,
    textChapter,
    comicChapter,
    audioTrack,
    audioTrackLoading,
    prepareOnlineReading,
    primeOnlineDirectory,
    loadDirectory,
    openOnlineTextChapter,
    peekOnlineTextChapter,
    openOnlineComicChapter,
    loadOnlineAudioTrack,
  };
}
