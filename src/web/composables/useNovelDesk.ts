import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import type { AuthStatus, Book, BrowserLoginStatus, Chapter, ChapterMode, ChapterVolume, Job, Novel, ViewName } from "../types";

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
  const auth = ref<AuthStatus>({ authenticated: false });
  const loginBusy = ref(false);
  const toast = ref("");
  let toastTimer: number | undefined;
  let loginPollTimer: number | undefined;

  const runningJobs = computed(() => jobs.value.filter((job) => ["downloading", "queued", "paused"].includes(job.status)));
  const completedJobs = computed(() => jobs.value.filter((job) => job.status === "done"));
  const libraryJobs = computed(() => jobs.value.filter((job) => ["queued", "downloading", "paused"].includes(job.status)));
  const bookshelfCategoryFor = (novel: Novel) => novel.bookshelfName || "未分类";
  const bookshelfCategories = computed(() => ["全部", ...Array.from(new Set([
    ...bookshelfGroupNames.value,
    ...bookshelf.value.map(bookshelfCategoryFor),
  ]))]);
  const filteredBookshelf = computed(() => bookshelfCategory.value === "全部" ? bookshelf.value : bookshelf.value.filter((novel) => bookshelfCategoryFor(novel) === bookshelfCategory.value));
  const bookshelfTotalPages = computed(() => Math.max(1, Math.ceil(filteredBookshelf.value.length / bookshelfPageSize)));
  const pagedBookshelf = computed(() => filteredBookshelf.value.slice((bookshelfPage.value - 1) * bookshelfPageSize, bookshelfPage.value * bookshelfPageSize));

  function notify(message: string) {
    toast.value = message;
    window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => { toast.value = ""; }, 2800);
  }

  async function refreshAuthStatus() {
    try { auth.value = await request<AuthStatus>("/api/auth/status"); } catch { /* server may be restarting */ }
  }

  async function pollBrowserLogin() {
    if (!loginBusy.value) return;
    try {
      const result = await request<BrowserLoginStatus>("/api/auth/browser-login/status");
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
    if (loginBusy.value) loginPollTimer = window.setTimeout(() => void pollBrowserLogin(), 1200);
  }

  async function login() {
    window.clearTimeout(loginPollTimer);
    loginBusy.value = true;
    try {
      await request<{ status: string }>("/api/auth/browser-login", { method: "POST" });
      notify("已打开官方登录窗口，请在窗口中完成登录和滑块验证");
      void pollBrowserLogin();
    } catch (error) {
      loginBusy.value = false;
      notify(error instanceof Error ? error.message : "无法打开官方登录窗口");
    }
  }

  async function logout() {
    try {
      window.clearTimeout(loginPollTimer);
      await fetch("/api/auth/logout", { method: "POST" });
      auth.value = { authenticated: false };
      notify("已清除本地登录会话");
    } catch { notify("退出登录失败"); }
  }

  async function search() {
    if (!query.value.trim()) return;
    await refreshAuthStatus();
    loading.value = true;
    searched.value = true;
    try {
      results.value = await request<Novel[]>(`/api/search?q=${encodeURIComponent(query.value.trim())}`);
    } catch (error) { notify(error instanceof Error ? error.message : "搜索失败"); }
    finally { loading.value = false; }
  }

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
        request<Partial<Novel>>(`/api/novel/${novel.novelId}`).catch(() => ({})),
        mode === "text"
          ? request<ChapterVolume[]>(`/api/chapters/${novel.novelId}`).catch(() => [])
          : Promise.resolve([] as ChapterVolume[]),
        auth.value.authenticated ? request<{ chapters: Chapter[] }>(`/api/audio/${novel.novelId}`).catch(() => ({ chapters: [] })) : Promise.resolve({ chapters: [] as Chapter[] }),
      ]);
      chapterNovel.value = { ...novel, ...info };
      chapterVolumes.value = volumes;
      audioChapters.value = audio.chapters;
      chapterHasAudio.value = audio.chapters.length > 0;
      if (mode === "audio" && !chapterHasAudio.value) chapterMode.value = "text";
      selectedChapterIds.value = allChapterIds().filter((id) => {
        const chapter = chapterMode.value === "text" ? chapterVolumes.value.flatMap((volume) => volume.chapters).find((item) => item.chapId === id) : audioChapters.value.find((item) => item.id === id);
        return chapter && !chapter.downloaded && (!chapter.isVip || chapter.isUnlocked);
      });
    } catch (error) {
      chapterModalOpen.value = false;
      notify(error instanceof Error ? error.message : "读取章节目录失败");
    } finally { chapterLoading.value = false; }
  }

  function allChapterIds() {
    return chapterMode.value === "text" ? chapterVolumes.value.flatMap((volume) => volume.chapters.map((chapter) => chapter.chapId)) : audioChapters.value.map((chapter) => chapter.id);
  }

  function changeChapterMode(mode: ChapterMode) {
    if (mode === "audio" && !chapterHasAudio.value) return;
    chapterMode.value = mode;
    selectedChapterIds.value = allChapterIds().filter((id) => {
      const chapter = mode === "text" ? chapterVolumes.value.flatMap((volume) => volume.chapters).find((item) => item.chapId === id) : audioChapters.value.find((item) => item.id === id);
      return chapter && !chapter.downloaded && (!chapter.isVip || chapter.isUnlocked);
    });
  }

  function toggleAllChapters() {
    const selectable = allChapterIds().filter((id) => {
      const chapter = chapterMode.value === "text" ? chapterVolumes.value.flatMap((volume) => volume.chapters).find((item) => item.chapId === id) : audioChapters.value.find((item) => item.id === id);
      return chapter && !chapter.downloaded && (!chapter.isVip || chapter.isUnlocked);
    });
    selectedChapterIds.value = selectedChapterIds.value.length === selectable.length ? [] : selectable;
  }

  async function confirmChapterDownload() {
    const novel = chapterNovel.value;
    if (!novel || !selectedChapterIds.value.length) return notify("请至少选择一章");
    try {
      const job = await request<Job>(chapterMode.value === "text" ? "/api/download" : "/api/audio/download", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ novelId: novel.novelId, title: novel.novelName, chapterIds: selectedChapterIds.value }) });
      jobs.value = [job, ...jobs.value];
      chapterModalOpen.value = false;
      notify(`已将《${novel.novelName}》加入下载队列`);
      void refreshJobs();
    } catch (error) { notify(error instanceof Error ? error.message : "创建下载任务失败"); }
  }

  async function pauseJob(job: Job) {
    try { await request<Job>(`/api/jobs/${job.id}/pause`, { method: "POST" }); job.status = "paused"; job.message = "已暂停，可继续下载"; } catch (error) { notify(error instanceof Error ? error.message : "暂停下载失败"); }
  }
  async function resumeJob(job: Job) {
    try { await request<Job>(`/api/jobs/${job.id}/resume`, { method: "POST" }); job.status = "downloading"; job.message = "正在继续下载"; void refreshJobs(); } catch (error) { notify(error instanceof Error ? error.message : "继续下载失败"); }
  }
  async function deleteJob(job: Job) {
    try { await request<void>(`/api/jobs/${job.id}`, { method: "DELETE" }); jobs.value = jobs.value.filter((item) => item.id !== job.id); } catch (error) { notify(error instanceof Error ? error.message : "删除任务失败"); }
  }
  function deleteBook(book: Book) { if (libraryManaging.value) confirmBook.value = book; }
  async function confirmDeleteBook() {
    const book = confirmBook.value;
    if (!book) return;
    try { await request<void>(`/api/library/${encodeURIComponent(book.name)}`, { method: "DELETE" }); library.value = library.value.filter((item) => item.name !== book.name); notify(`已删除《${book.name}》本地内容`); } catch (error) { notify(error instanceof Error ? error.message : "删除本地内容失败"); }
    confirmBook.value = undefined;
  }
  async function openLibraryBook(book: Book) {
    if (book.novelId) return openChapterPicker({ novelId: book.novelId, novelName: book.name, authorName: "", novelCover: book.cover || "", lastUpdateTime: book.updatedAt }, "text");
    try {
      const matches = await request<Novel[]>(`/api/search?q=${encodeURIComponent(book.name)}`);
      const match = matches.find((novel) => novel.novelName === book.name) || matches[0];
      if (match) return openChapterPicker(match, "text");
    } catch { /* retain fallback below */ }
    notify("暂时找不到对应的在线小说信息");
  }

  function selectBookshelfCategory(category: string) { bookshelfCategory.value = category; bookshelfPage.value = 1; }
  function setBookshelfPage(page: number) { bookshelfPage.value = Math.min(Math.max(page, 1), bookshelfTotalPages.value); }
  async function refreshJobs() {
    try {
      jobs.value = await request<Job[]>("/api/jobs");
      if (runningJobs.value.length) window.setTimeout(() => void refreshJobs(), 1000);
      if (completedJobs.value.length) void refreshLibrary();
    } catch { /* server may be restarting */ }
  }
  async function refreshLibrary() { try { library.value = await request<Book[]>("/api/library"); } catch { /* no library yet */ } }
  async function refreshBookshelf(forceRefresh = false) {
    await refreshAuthStatus();
    if (!auth.value.authenticated) { credentialsOpen.value = true; notify("登录后即可读取 SF 书架"); return; }
    bookshelfLoading.value = true;
    try {
      const response = await request<BookshelfResponse>(`/api/bookshelf${forceRefresh ? "?refresh=1" : ""}`);
      bookshelf.value = response.items;
      bookshelfGroupNames.value = response.categories;
      if (!bookshelfCategories.value.includes(bookshelfCategory.value)) bookshelfCategory.value = "全部";
      bookshelfPage.value = 1;
    } catch (error) { notify(error instanceof Error ? error.message : "读取书架失败"); }
    finally { bookshelfLoading.value = false; }
  }
  function navigate(view: ViewName) { active.value = view; if (view === "bookshelf") void refreshBookshelf(); }
  function formatDate(value?: string) {
    if (!value) return "未知时间";
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? "未知时间" : new Intl.DateTimeFormat("zh-CN", { month: "short", day: "numeric" }).format(date);
  }
  function blockCopy(event: ClipboardEvent) { event.preventDefault(); }

  onMounted(() => { void refreshJobs(); void refreshLibrary(); void refreshAuthStatus(); document.addEventListener("copy", blockCopy); });
  onBeforeUnmount(() => { window.clearTimeout(loginPollTimer); window.clearTimeout(toastTimer); document.removeEventListener("copy", blockCopy); });

  return { active, query, results, loading, searched, jobs, library, bookshelf, bookshelfLoading, bookshelfCategory, bookshelfPage, libraryManaging, confirmBook, chapterModalOpen, chapterLoading, chapterMode, chapterHasAudio, chapterNovel, chapterVolumes, audioChapters, selectedChapterIds, queueOpen, credentialsOpen, auth, loginBusy, toast, runningJobs, libraryJobs, bookshelfCategories, filteredBookshelf, bookshelfTotalPages, pagedBookshelf, search, openChapterPicker, changeChapterMode, toggleAllChapters, confirmChapterDownload, pauseJob, resumeJob, deleteJob, deleteBook, confirmDeleteBook, openLibraryBook, selectBookshelfCategory, setBookshelfPage, refreshBookshelf, navigate, login, logout, formatDate };
}
