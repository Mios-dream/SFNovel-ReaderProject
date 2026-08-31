import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import type {
  AuthStatus,
  Book,
  Chapter,
  ChapterMode,
  ChapterVolume,
  Job,
  LocalBookDetail,
  LocalChapterContent,
  Novel,
  RequestPolicy,
  UserProfile,
  ViewName,
} from "../types";

type BookshelfResponse = { categories: string[]; items: Novel[] };

function localAssetSource(path?: string) {
  if (!path || !isTauri()) return path;
  return convertFileSrc(path);
}

/**
 * 创建小说桌面页面使用的共享响应式状态与操作集合。
 * @returns 响应式页面状态、计算属性和用户操作方法。
 */
export function useNovelDesk() {
  const active = ref<ViewName>("discover");
  const query = ref("");
  const results = ref<Novel[]>([]);
  const loading = ref(false);
  const searched = ref(false);
  const jobs = ref<Job[]>([]);
  const library = ref<Book[]>([]);
  const bookshelf = ref<Novel[]>([]);
  const bookshelfGroupNames = ref<string[]>([]);
  const bookshelfLoading = ref(false);
  const bookshelfCategory = ref("全部");
  const bookshelfPage = ref(1);
  const bookshelfPageSize = 12;
  const libraryManaging = ref(false);
  const confirmBook = ref<Book>();
  const localBook = ref<LocalBookDetail>();
  const localChapter = ref<LocalChapterContent>();
  const localAudioTrackIndex = ref(0);
  const exportingFormat = ref<"epub" | "markdown" | "txt" | "audio">();
  const chapterModalOpen = ref(false);
  const chapterLoading = ref(false);
  const chapterMode = ref<ChapterMode>("text");
  const chapterHasAudio = ref(false);
  const chapterNovel = ref<Novel>();
  const chapterVolumes = ref<ChapterVolume[]>([]);
  const audioChapters = ref<Chapter[]>([]);
  const selectedChapterIds = ref<number[]>([]);
  const queueOpen = ref(false);
  const credentialsOpen = ref(false);
  const accountOpen = ref(false);
  const accountProfile = ref<UserProfile>();
  const accountProfileLoading = ref(false);
  const accountProfileError = ref("");
  const requestPolicyOpen = ref(false);
  const requestPolicy = ref<RequestPolicy>({
    requestIntervalMs: 500,
    maxConcurrentDownloads: 1,
    webFallbackEnabled: true,
  });
  const requestPolicySaving = ref(false);
  const contentDictionarySize = ref(0);
  const contentDictionaryUpdating = ref(false);
  const contentDictionaryChapterId = ref(8436696);
  const auth = ref<AuthStatus>({ authenticated: false });
  const loginBusy = ref(false);
  const toast = ref("");
  let toastTimer: number | undefined;
  let stopJobListener: (() => void) | undefined;

  const runningJobs = computed(() =>
    jobs.value.filter((job) =>
      ["downloading", "queued", "paused"].includes(job.status),
    ),
  );
  const completedJobs = computed(() =>
    jobs.value.filter((job) => job.status === "done"),
  );
  const libraryJobs = computed(() =>
    jobs.value.filter((job) =>
      ["queued", "downloading", "paused"].includes(job.status),
    ),
  );
  /**
   * Resolves the display category for one bookshelf novel.
   *
   * @param novel Novel whose server-provided bookshelf name is inspected.
   * @returns The bookshelf name, or the stable fallback `未分类`.
   */
  const bookshelfCategoryFor = (novel: Novel) =>
    novel.bookshelfName || "未分类";
  const bookshelfCategories = computed(() => [
    "全部",
    ...Array.from(
      new Set([
        ...bookshelfGroupNames.value,
        ...bookshelf.value.map(bookshelfCategoryFor),
      ]),
    ),
  ]);
  const filteredBookshelf = computed(() =>
    bookshelfCategory.value === "全部"
      ? bookshelf.value
      : bookshelf.value.filter(
          (novel) => bookshelfCategoryFor(novel) === bookshelfCategory.value,
        ),
  );
  const bookshelfTotalPages = computed(() =>
    Math.max(1, Math.ceil(filteredBookshelf.value.length / bookshelfPageSize)),
  );
  const pagedBookshelf = computed(() =>
    filteredBookshelf.value.slice(
      (bookshelfPage.value - 1) * bookshelfPageSize,
      bookshelfPage.value * bookshelfPageSize,
    ),
  );

  /**
   * 显示短时提示并重置自动隐藏计时器。
   * @param message 要显示给用户的提示文本。
   * @returns 无返回值。
   */
  function notify(message: string) {
    toast.value = message;
    window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => {
      toast.value = "";
    }, 2800);
  }

  /**
   * 从 native Rust 会话层刷新当前 SF 登录状态。
   * @returns 请求完成后的 Promise。
   */
  async function refreshAuthStatus() {
    try {
      auth.value = await invoke<AuthStatus>("auth_status");
    } catch {
      /* Native runtime may be restarting during development. */
    }
  }

  /**
   * Loads the current account profile for both the sidebar identity and profile modal.
   *
   * @returns A promise settled after the native profile request finishes.
   */
  async function loadAccountProfile() {
    if (!auth.value.authenticated) return;
    accountProfileLoading.value = true;
    accountProfileError.value = "";
    try {
      const profile = await invoke<UserProfile>("get_user_profile");
      accountProfile.value = profile;
      auth.value = { ...auth.value, userName: profile.nickName };
    } catch (error) {
      accountProfileError.value = nativeErrorMessage(error, "读取账户资料失败");
    } finally {
      accountProfileLoading.value = false;
    }
  }

  /**
   * 将用户输入的账号密码提交给原生请求层进行认证。
   * @returns 登录窗口启动请求完成后的 Promise。
   */
  async function login(username: string, password: string) {
    loginBusy.value = true;
    try {
      const result = await invoke<AuthStatus>("login_with_password", {
        username,
        password,
      });
      auth.value = result;
      credentialsOpen.value = false;
      await loadAccountProfile();
      notify("SF 账号已登录到当前会话");
    } catch (error) {
      notify(nativeErrorMessage(error, "SF 账号密码登录失败"));
    } finally {
      loginBusy.value = false;
    }
  }

  /**
   * 启动平台原生的官方网页登录流程。
   * @returns 官方登录流程完成后的 Promise。
   */
  async function browserLogin() {
    loginBusy.value = true;
    try {
      await invoke<void>("start_official_login");
      const result = await invoke<AuthStatus>("auth_status");
      if (!result.authenticated) {
        notify("未检测到官方登录会话");
        return;
      }
      auth.value = result;
      credentialsOpen.value = false;
      await loadAccountProfile();
      notify("SF 账号已登录到当前会话");
    } catch (error) {
      notify(nativeErrorMessage(error, "无法打开官方登录窗口"));
    } finally {
      loginBusy.value = false;
    }
  }

  /**
   * Keeps native command failures visible in both the UI and DevTools.
   * Tauri may reject an invoke with a plain string rather than an Error object.
   */
  function nativeErrorMessage(error: unknown, fallback: string) {
    console.error(`[SF Novel Flow] ${fallback}`, error);
    return typeof error === "string"
      ? error
      : error instanceof Error && error.message
        ? error.message
        : fallback;
  }

  /**
   * 清除 native 会话和 Android 应用内 WebView Cookie。
   * @returns 登出请求完成后的 Promise。
   */
  async function logout() {
    try {
      await invoke<void>("logout");
      auth.value = { authenticated: false };
      accountOpen.value = false;
      credentialsOpen.value = false;
      accountProfile.value = undefined;
      accountProfileError.value = "";
      notify("已清除本地登录会话");
    } catch {
      notify("退出登录失败");
    }
  }

  /**
   * Opens account information or the credential modal when signed out.
   *
   * @returns A promise settled after profile loading and state synchronization.
   */
  async function openAccount() {
    if (!auth.value.authenticated) {
      credentialsOpen.value = true;
      return;
    }
    accountOpen.value = true;
    await loadAccountProfile();
  }

  /**
   * 打开请求设置弹窗并读取当前策略。
   * @returns 设置读取完成后的 Promise。
   */
  async function openRequestPolicy() {
    requestPolicyOpen.value = true;
    try {
      requestPolicy.value = await invoke<RequestPolicy>("get_request_policy");
      const dictionary = await invoke<{ size: number }>("get_content_dictionary");
      contentDictionarySize.value = dictionary.size;
    } catch (error) {
      notify(error instanceof Error ? error.message : "读取请求设置失败");
    }
  }

  /**
   * 使用指定公开章节对齐 API 混淆字与网页正文，并保存 native 字典。
   * @param chapterId 用于对齐的正整数章节编号。
   * @returns 字典更新完成后的 Promise。
   */
  async function updateContentDictionary(chapterId: number) {
    if (!Number.isInteger(chapterId) || chapterId <= 0) {
      notify("请输入有效的章节编号");
      return;
    }
    contentDictionaryChapterId.value = chapterId;
    contentDictionaryUpdating.value = true;
    try {
      const result = await invoke<{ size: number; added?: number }>(
        "update_content_dictionary",
        { chapterId },
      );
      contentDictionarySize.value = result.size;
      notify(`正文恢复字典已更新，新增 ${result.added ?? 0} 个字符`);
    } catch (error) {
      notify(error instanceof Error ? error.message : "正文恢复字典更新失败");
    } finally {
      contentDictionaryUpdating.value = false;
    }
  }

  /**
   * 保存请求限流策略并关闭设置弹窗。
   * @param policy 要保存的请求间隔和并发数。
   * @returns 保存请求完成后的 Promise。
   */
  async function saveRequestPolicy(policy: RequestPolicy) {
    requestPolicySaving.value = true;
    try {
      requestPolicy.value = await invoke<RequestPolicy>("save_request_policy", {
        policy,
      });
      requestPolicyOpen.value = false;
      notify("请求设置已更新");
    } catch (error) {
      notify(error instanceof Error ? error.message : "保存请求设置失败");
    } finally {
      requestPolicySaving.value = false;
    }
  }

  /**
   * 使用当前搜索关键词查询 SF 作品。
   * @returns 搜索请求完成后的 Promise。
   */
  async function search() {
    if (!query.value.trim()) return;
    await refreshAuthStatus();
    loading.value = true;
    searched.value = true;
    try {
      results.value = await invoke<Novel[]>("search_novels", {
        query: query.value.trim(),
      });
    } catch (error) {
      notify(nativeErrorMessage(error, "搜索失败"));
    } finally {
      loading.value = false;
    }
  }

  /**
   * 加载作品目录并打开章节选择弹窗。
   * @param novel 要查看或下载的作品。
   * @param mode 初始章节类型，文本或有声。
   * @returns 目录加载完成后的 Promise。
   */
  async function openChapterPicker(novel: Novel, mode: ChapterMode) {
    // 并行加载作品详情、文本目录和有声目录；有声目录由 native 层校验会话。
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
      selectedChapterIds.value = allChapterIds().filter((id) => {
        const chapter =
          chapterMode.value === "text"
            ? chapterVolumes.value
                .flatMap((volume) => volume.chapters)
                .find((item) => item.chapId === id)
            : audioChapters.value.find((item) => item.id === id);
        return (
          chapter &&
          !chapter.downloaded &&
          (!chapter.isVip || chapter.isUnlocked)
        );
      });
    } catch (error) {
      chapterModalOpen.value = false;
      notify(error instanceof Error ? error.message : "读取章节目录失败");
    } finally {
      chapterLoading.value = false;
    }
  }

  /**
   * 获取当前目录模式下的全部章节 ID。
   * @returns 当前文本或有声目录中的章节 ID 数组。
   */
  function allChapterIds(): number[] {
    return chapterMode.value === "text"
      ? chapterVolumes.value.flatMap((volume) =>
          volume.chapters.map((chapter) => chapter.chapId),
        )
      : audioChapters.value.map((chapter) => chapter.id);
  }

  /**
   * 切换章节类型并重新选择该类型下可下载的章节。
   * @param mode 目标章节类型。
   * @returns 无返回值。
   */
  function changeChapterMode(mode: ChapterMode) {
    if (mode === "audio" && !chapterHasAudio.value) return;
    chapterMode.value = mode;
    selectedChapterIds.value = allChapterIds().filter((id) => {
      const chapter =
        mode === "text"
          ? chapterVolumes.value
              .flatMap((volume) => volume.chapters)
              .find((item) => item.chapId === id)
          : audioChapters.value.find((item) => item.id === id);
      return (
        chapter &&
        !chapter.downloaded &&
        (!chapter.isVip || chapter.isUnlocked)
      );
    });
  }

  /**
   * 在全选和取消全选之间切换当前目录的可下载章节。
   * @returns 无返回值。
   */
  function toggleAllChapters() {
    // 只在可下载章节集合中全选/取消全选，已下载或未解锁章节不会加入任务。
    const selectable = allChapterIds().filter((id) => {
      const chapter =
        chapterMode.value === "text"
          ? chapterVolumes.value
              .flatMap((volume) => volume.chapters)
              .find((item) => item.chapId === id)
          : audioChapters.value.find((item) => item.id === id);
      return (
        chapter &&
        !chapter.downloaded &&
        (!chapter.isVip || chapter.isUnlocked)
      );
    });
    selectedChapterIds.value =
      selectedChapterIds.value.length === selectable.length ? [] : selectable;
  }

  /**
   * 将选中的章节提交为文本或有声下载任务。
   * @returns 创建任务请求完成后的 Promise。
   */
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
      jobs.value = [job, ...jobs.value];
      chapterModalOpen.value = false;
      notify(`已将《${novel.novelName}》加入下载队列`);
      void refreshJobs();
    } catch (error) {
      notify(error instanceof Error ? error.message : "创建下载任务失败");
    }
  }

  /**
   * 暂停指定的后台下载任务。
   * @param job 要暂停的任务。
   * @returns 暂停请求完成后的 Promise。
   */
  async function pauseJob(job: Job) {
    try {
      await invoke<Job>("pause_download_job", { jobId: job.id });
      job.status = "paused";
      job.message = "已暂停，可继续下载";
    } catch (error) {
      notify(error instanceof Error ? error.message : "暂停下载失败");
    }
  }
  /**
   * 恢复指定的后台下载任务。
   * @param job 要恢复的任务。
   * @returns 恢复请求完成后的 Promise。
   */
  async function resumeJob(job: Job) {
    try {
      await invoke<Job>("resume_download_job", { jobId: job.id });
      job.status = "downloading";
      job.message = "正在继续下载";
      void refreshJobs();
    } catch (error) {
      notify(error instanceof Error ? error.message : "继续下载失败");
    }
  }
  /**
   * 删除任务记录并从当前列表移除。
   * @param job 要删除的任务。
   * @returns 删除请求完成后的 Promise。
   */
  async function deleteJob(job: Job) {
    try {
      await invoke<void>("delete_download_job", { jobId: job.id });
      jobs.value = jobs.value.filter((item) => item.id !== job.id);
    } catch (error) {
      notify(error instanceof Error ? error.message : "删除任务失败");
    }
  }
  /**
   * 在管理模式下打开本地书籍删除确认框。
   * @param book 要删除的本地书籍。
   * @returns 无返回值。
   */
  function deleteBook(book: Book) {
    if (libraryManaging.value) confirmBook.value = book;
  }
  /**
   * 删除确认框中选定的本地书籍目录。
   * @returns 删除请求完成后的 Promise。
   */
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
  /**
   * Loads a local book's metadata and opens its downloaded chapter list.
   *
   * @param book Local library entry selected by the user.
   * @returns A promise settled after the native library lookup.
   */
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
      active.value = "libraryDetail";
    } catch (error) {
      notify(error instanceof Error ? error.message : "无法读取本地书籍详情");
    }
  }

  /**
   * Loads one downloaded text chapter and navigates to the local reader.
   *
   * @param chapterId Persisted positive chapter identifier.
   * @returns A promise settled after the native chapter lookup.
   */
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

  /**
   * Opens the local audio player at a bounded track index.
   *
   * @param trackIndex Optional zero-based track index requested by the caller.
   * @returns No value; shows a toast instead when no local audio exists.
   */
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

  /**
   * Opens the selected novel in SF's official external browser page.
   *
   * @returns A promise settled after the opener request is dispatched; does
   * nothing when the local book has no associated novel ID.
   */
  async function readOnline() {
    const novelId = localBook.value?.novelId;
    if (!novelId) return;
    const url = `https://book.sfacg.com/Novel/${novelId}/`;
    await openUrl(url);
  }

  /**
   * Lets the user choose a destination, then writes the native export there.
   *
   * @param format Export format supported by the native command.
   * @returns A promise settled after export and opener dispatch complete.
   */
  async function exportBook(format: "epub" | "markdown" | "txt" | "audio") {
    const book = localBook.value;
    if (!book) return;
    exportingFormat.value = format;
    try {
      const extension = format === "epub" ? "epub" : format === "txt" ? "txt" : "zip";
      const suffix =
        format === "markdown" ? "-Markdown" : format === "audio" ? "-有声" : "";
      const defaultPath = `${book.name}${suffix}.${extension}`;
      const selectedPath = await save({
        title: "选择导出位置",
        defaultPath,
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

  /**
   * Reopens chapter selection for the current local book's online content.
   *
   * @returns No value; navigation occurs only when a novel ID is available.
   */
  function continueDownload() {
    const book = localBook.value;
    if (!book?.novelId) return;
    void openChapterPicker(
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

  /**
   * 选择书架分类并回到第一页。
   * @param category 目标书架分类名称。
   * @returns 无返回值。
   */
  function selectBookshelfCategory(category: string) {
    bookshelfCategory.value = category;
    bookshelfPage.value = 1;
  }
  /**
   * 设置书架页码，并将页码限制在有效范围内。
   * @param page 请求跳转的页码。
   * @returns 无返回值。
   */
  function setBookshelfPage(page: number) {
    bookshelfPage.value = Math.min(
      Math.max(page, 1),
      bookshelfTotalPages.value,
    );
  }
  /**
   * 刷新 native 任务列表；存在运行中任务时继续轮询。
   * @returns 任务列表请求完成后的 Promise。
   */
  async function refreshJobs() {
    // 任务状态由 native event 推送；这里仅用于启动和手动刷新时读取快照。
    try {
      jobs.value = await invoke<Job[]>("list_download_jobs");
      if (completedJobs.value.length) void refreshLibrary();
    } catch {
      /* Native runtime may be restarting during development. */
    }
  }
  /**
   * 从 native 本地书库服务刷新索引。
   * @returns 书库请求完成后的 Promise。
   */
  async function refreshLibrary() {
    try {
      const books = await invoke<Book[]>("list_local_library");
      library.value = books.map((book) => ({
        ...book,
        cover: localAssetSource(book.cover),
      }));
    } catch {
      /* no library yet */
    }
  }
  /**
   * 读取当前账号书架并更新分类、分页状态。
   * @param forceRefresh 是否跳过 native 书架缓存重新请求。
   * @returns 书架请求完成后的 Promise。
   */
  async function refreshBookshelf(forceRefresh = false) {
    await refreshAuthStatus();
    if (!auth.value.authenticated) {
      credentialsOpen.value = true;
      notify("登录后即可读取 SF 书架");
      return;
    }
    bookshelfLoading.value = true;
    try {
      const response = await invoke<BookshelfResponse>("get_bookshelf", {
        forceRefresh,
      });
      bookshelf.value = response.items;
      bookshelfGroupNames.value = response.categories;
      if (!bookshelfCategories.value.includes(bookshelfCategory.value))
        bookshelfCategory.value = "全部";
      bookshelfPage.value = 1;
    } catch (error) {
      notify(nativeErrorMessage(error, "读取书架失败"));
    } finally {
      bookshelfLoading.value = false;
    }
  }
  /**
   * 切换主界面页面，进入书架时自动触发同步。
   * @param view 目标页面名称。
   * @returns 无返回值。
   */
  function navigate(view: ViewName) {
    active.value = view;
    if (view === "bookshelf") void refreshBookshelf();
  }
  /**
   * 将 ISO 日期格式化为中文月日显示文本。
   * @param value 可选的日期字符串。
   * @returns 格式化后的日期或“未知时间”。
   */
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
  /**
   * 阻止页面复制事件，用于保护阅读界面的内容展示。
   * @param event 浏览器复制事件。
   * @returns 无返回值。
   */
  function blockCopy(event: ClipboardEvent) {
    event.preventDefault();
  }

  onMounted(() => {
    void listen<Job>("download-progress", (event) => {
      const index = jobs.value.findIndex((job) => job.id === event.payload.id);
      if (index < 0) jobs.value.unshift(event.payload);
      else jobs.value[index] = event.payload;
      if (event.payload.status === "done") void refreshLibrary();
    }).then((unlisten) => {
      stopJobListener = unlisten;
    });
    void refreshJobs();
    void refreshLibrary();
    void refreshAuthStatus().then(() => loadAccountProfile());
    document.addEventListener("copy", blockCopy);
  });
  onBeforeUnmount(() => {
    stopJobListener?.();
    window.clearTimeout(toastTimer);
    document.removeEventListener("copy", blockCopy);
  });

  return {
    active,
    query,
    results,
    loading,
    searched,
    jobs,
    library,
    bookshelf,
    bookshelfLoading,
    bookshelfCategory,
    bookshelfPage,
    libraryManaging,
    confirmBook,
    localBook,
    localChapter,
    localAudioTrackIndex,
    exportingFormat,
    chapterModalOpen,
    chapterLoading,
    chapterMode,
    chapterHasAudio,
    chapterNovel,
    chapterVolumes,
    audioChapters,
    selectedChapterIds,
    queueOpen,
    credentialsOpen,
    accountOpen,
    accountProfile,
    accountProfileLoading,
    accountProfileError,
    requestPolicyOpen,
    requestPolicy,
    requestPolicySaving,
    contentDictionarySize,
    contentDictionaryUpdating,
    contentDictionaryChapterId,
    auth,
    loginBusy,
    toast,
    runningJobs,
    libraryJobs,
    bookshelfCategories,
    filteredBookshelf,
    bookshelfTotalPages,
    pagedBookshelf,
    search,
    openChapterPicker,
    changeChapterMode,
    toggleAllChapters,
    confirmChapterDownload,
    pauseJob,
    resumeJob,
    deleteJob,
    deleteBook,
    confirmDeleteBook,
    openLibraryBook,
    openLocalChapter,
    openLocalAudioPlayer,
    readOnline,
    exportBook,
    continueDownload,
    selectBookshelfCategory,
    setBookshelfPage,
    refreshBookshelf,
    navigate,
    login,
    browserLogin,
    logout,
    openAccount,
    openRequestPolicy,
    saveRequestPolicy,
    updateContentDictionary,
    formatDate,
  };
}
