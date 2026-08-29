import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import type {
  AuthStatus,
  Book,
  BrowserLoginStatus,
  Chapter,
  ChapterMode,
  ChapterVolume,
  Job,
  Novel,
  RequestPolicy,
  ViewName,
} from "../types";

/**
 * 发起同源 JSON 请求并统一转换后端错误。
 * @param url API 路径。
 * @param options 可选的 Fetch 请求配置。
 * @returns 解析后的响应数据。
 * @throws 当响应状态不是成功状态时抛出后端错误消息。
 */
async function request<T>(url: string, options?: RequestInit): Promise<T> {
  const response = await fetch(url, { credentials: "same-origin", ...options });
  if (!response.ok) {
    const data = await response.json().catch(() => ({}));
    throw new Error(data.message || "请求未完成");
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

type BookshelfResponse = { categories: string[]; items: Novel[] };

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
  const requestPolicyOpen = ref(false);
  const requestPolicy = ref<RequestPolicy>({
    requestIntervalMs: 500,
    maxConcurrentDownloads: 1,
  });
  const requestPolicySaving = ref(false);
  const auth = ref<AuthStatus>({ authenticated: false });
  const loginBusy = ref(false);
  const toast = ref("");
  let toastTimer: number | undefined;
  let loginPollTimer: number | undefined;

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
   * 从服务端刷新当前 SF 登录状态。
   * @returns 请求完成后的 Promise。
   */
  async function refreshAuthStatus() {
    try {
      auth.value = await request<AuthStatus>("/api/auth/status");
    } catch {
      /* server may be restarting */
    }
  }

  /**
   * 轮询受控浏览器登录结果，成功后更新当前会话。
   * @returns 轮询步骤完成后的 Promise。
   */
  async function pollBrowserLogin() {
    // 登录在官方窗口完成，前端通过短轮询等待后端提取会话 Cookie。
    if (!loginBusy.value) return;
    try {
      const result = await request<BrowserLoginStatus>(
        "/api/auth/browser-login/status",
      );
      if (result.authenticated) {
        auth.value = result;
        loginBusy.value = false;
        credentialsOpen.value = false;
        notify("SF 账号已登录到当前会话");
        return;
      }
      if (!result.waiting) {
        loginBusy.value = false;
        notify("官方登录窗口未运行，请重新打开");
        return;
      }
    } catch (error) {
      loginBusy.value = false;
      notify(error instanceof Error ? error.message : "官方登录失败");
    }
    if (loginBusy.value)
      loginPollTimer = window.setTimeout(() => void pollBrowserLogin(), 1200);
  }

  /**
   * 请求后端打开官方登录浏览器并开始状态轮询。
   * @returns 登录窗口启动请求完成后的 Promise。
   */
  async function login() {
    window.clearTimeout(loginPollTimer);
    loginBusy.value = true;
    try {
      await request<{ status: string }>("/api/auth/browser-login", {
        method: "POST",
      });
      notify("已打开官方登录窗口，请在窗口中完成登录和滑块验证");
      void pollBrowserLogin();
    } catch (error) {
      loginBusy.value = false;
      notify(error instanceof Error ? error.message : "无法打开官方登录窗口");
    }
  }

  /**
   * 清除服务端保存的当前登录会话。
   * @returns 登出请求完成后的 Promise。
   */
  async function logout() {
    try {
      window.clearTimeout(loginPollTimer);
      await fetch("/api/auth/logout", { method: "POST" });
      auth.value = { authenticated: false };
      notify("已清除本地登录会话");
    } catch {
      notify("退出登录失败");
    }
  }

  /**
   * 打开请求设置弹窗并读取当前策略。
   * @returns 设置读取完成后的 Promise。
   */
  async function openRequestPolicy() {
    requestPolicyOpen.value = true;
    try {
      requestPolicy.value = await request<RequestPolicy>(
        "/api/settings/request-policy",
      );
    } catch (error) {
      notify(error instanceof Error ? error.message : "读取请求设置失败");
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
      requestPolicy.value = await request<RequestPolicy>(
        "/api/settings/request-policy",
        {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(policy),
        },
      );
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
      results.value = await request<Novel[]>(
        `/api/search?q=${encodeURIComponent(query.value.trim())}`,
      );
    } catch (error) {
      notify(error instanceof Error ? error.message : "搜索失败");
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
    // 并行加载作品详情、文本目录和有声目录；未登录时跳过有声请求。
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
        request<Partial<Novel>>(`/api/novel/${novel.novelId}`).catch(
          () => ({}),
        ),
        mode === "text"
          ? request<ChapterVolume[]>(`/api/chapters/${novel.novelId}`).catch(
              () => [],
            )
          : Promise.resolve([] as ChapterVolume[]),
        auth.value.authenticated
          ? request<{ chapters: Chapter[] }>(
              `/api/audio/${novel.novelId}`,
            ).catch(() => ({ chapters: [] }))
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
        chapter && !chapter.downloaded && (!chapter.isVip || chapter.isUnlocked)
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
        chapter && !chapter.downloaded && (!chapter.isVip || chapter.isUnlocked)
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
      const job = await request<Job>(
        chapterMode.value === "text" ? "/api/download" : "/api/audio/download",
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            novelId: novel.novelId,
            title: novel.novelName,
            chapterIds: selectedChapterIds.value,
          }),
        },
      );
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
      await request<Job>(`/api/jobs/${job.id}/pause`, { method: "POST" });
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
      await request<Job>(`/api/jobs/${job.id}/resume`, { method: "POST" });
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
      await request<void>(`/api/jobs/${job.id}`, { method: "DELETE" });
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
      await request<void>(`/api/library/${encodeURIComponent(book.name)}`, {
        method: "DELETE",
      });
      library.value = library.value.filter((item) => item.name !== book.name);
      notify(`已删除《${book.name}》本地内容`);
    } catch (error) {
      notify(error instanceof Error ? error.message : "删除本地内容失败");
    }
    confirmBook.value = undefined;
  }
  /**
   * 打开本地书籍对应的在线章节详情；缺少 ID 时先按书名搜索。
   * @param book 要打开的本地书籍。
   * @returns 目录加载或搜索请求完成后的 Promise。
   */
  async function openLibraryBook(book: Book) {
    if (book.novelId)
      return openChapterPicker(
        {
          novelId: book.novelId,
          novelName: book.name,
          authorName: "",
          novelCover: book.cover || "",
          lastUpdateTime: book.updatedAt,
        },
        "text",
      );
    try {
      const matches = await request<Novel[]>(
        `/api/search?q=${encodeURIComponent(book.name)}`,
      );
      const match =
        matches.find((novel) => novel.novelName === book.name) || matches[0];
      if (match) return openChapterPicker(match, "text");
    } catch {
      /* retain fallback below */
    }
    notify("暂时找不到对应的在线小说信息");
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
   * 刷新任务列表；存在运行中任务时继续轮询。
   * @returns 任务列表请求完成后的 Promise。
   */
  async function refreshJobs() {
    // 下载进行中保持轮询，任务完成后刷新本地书库列表。
    try {
      jobs.value = await request<Job[]>("/api/jobs");
      if (runningJobs.value.length)
        window.setTimeout(() => void refreshJobs(), 1000);
      if (completedJobs.value.length) void refreshLibrary();
    } catch {
      /* server may be restarting */
    }
  }
  /**
   * 从服务端刷新本地书库索引。
   * @returns 书库请求完成后的 Promise。
   */
  async function refreshLibrary() {
    try {
      library.value = await request<Book[]>("/api/library");
    } catch {
      /* no library yet */
    }
  }
  /**
   * 读取当前账号书架并更新分类、分页状态。
   * @param forceRefresh 是否跳过服务端缓存重新请求。
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
      const response = await request<BookshelfResponse>(
        `/api/bookshelf${forceRefresh ? "?refresh=1" : ""}`,
      );
      bookshelf.value = response.items;
      bookshelfGroupNames.value = response.categories;
      if (!bookshelfCategories.value.includes(bookshelfCategory.value))
        bookshelfCategory.value = "全部";
      bookshelfPage.value = 1;
    } catch (error) {
      notify(error instanceof Error ? error.message : "读取书架失败");
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
    void refreshJobs();
    void refreshLibrary();
    void refreshAuthStatus();
    document.addEventListener("copy", blockCopy);
  });
  onBeforeUnmount(() => {
    window.clearTimeout(loginPollTimer);
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
    requestPolicyOpen,
    requestPolicy,
    requestPolicySaving,
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
    selectBookshelfCategory,
    setBookshelfPage,
    refreshBookshelf,
    navigate,
    login,
    logout,
    openRequestPolicy,
    saveRequestPolicy,
    formatDate,
  };
}
