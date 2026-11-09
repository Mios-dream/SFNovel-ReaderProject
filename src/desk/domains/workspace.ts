import { ref } from "vue";
import type {
  LocalWorkMetadata,
  MediaType,
  Novel,
  ReaderSource,
  ViewName,
} from "../../types";
import type { BookshelfDesk } from "./bookshelf";
import type { LibraryDesk } from "./library";
import type { OnlineDesk } from "./online";
import type { PickerDesk } from "./picker";
import {
  currentLayer,
  currentView,
  isReaderView,
  pushComments,
  pushView,
  returnViews,
  type RouteLayer,
} from "../navigation";
import { dismissTopLayer } from "../backLayers";
import type { RemoteDesk } from "./remote";

type WorkspaceDeskOptions = {
  library: LibraryDesk;
  bookshelf: BookshelfDesk;
  online: OnlineDesk;
  picker: PickerDesk;
  remote: RemoteDesk;
};

/** 本地作品条目到远端作品标识的映射：有声作品的目录编号与专辑编号不同。 */
function workIdentity(work: LocalWorkMetadata, mode: MediaType) {
  return mode === "audio"
    ? { novelId: work.catalogId || work.id, mediaId: work.id }
    : { novelId: work.id, mediaId: undefined };
}

/**
 * 用已下载清单重建一份远端作品条目。
 *
 * 续传与「打开在线版本」都需要它：本地只存了标识与标题，而章节弹窗与在线阅读
 * 要的是上游作品对象。`lastUpdateTime` 仅用于展示，缺省时留空。
 */
function novelFromLocalWork(
  work: LocalWorkMetadata,
  mode: MediaType,
  lastUpdateTime = "",
): Novel {
  return {
    ...workIdentity(work, mode),
    novelName: work.title,
    authorName: work.author,
    novelCover: work.cover || "",
    lastUpdateTime,
  };
}

/**
 * 跨域流程与导航。
 *
 * 这里是唯一允许同时依赖多个域的地方：进入/离开阅读、续传、返回键与启动初始化都
 * 需要多个域协作。域内部的规则仍然留在各自模块里，本层只做编排与路由跳转。
 */
export function createWorkspaceDesk({
  library,
  bookshelf,
  online,
  picker,
  remote,
}: WorkspaceDeskOptions) {
  const readerSource = ref<ReaderSource>("local");

  /**
   * 进入某个视图前的状态前置。
   *
   * 只改状态、不改路由：返回键与「已经在目标页」的场景同样需要它，但不应触发
   * 一次重复导航。离开远端详情会清会话，进入阅读页则把来源标记为本地。
   */
  function prepare(view: ViewName) {
    if (remote.isSessionActive() && view !== "remoteDetail")
      remote.clearSession();
    if (isReaderView(view)) readerSource.value = "local";
    if (view === "bookshelf") void bookshelf.loadBookshelf();
    if (view === "library") {
      void library.ensureLibraryAssetAccess().then((granted) => {
        if (granted) void library.refreshLibrary();
      });
    }
  }

  /**
   * 唯一的视图跳转入口：先前置、再改路由。
   * @param bookName 详情页的目标书名，缺省时取当前已加载的本地书籍。
   */
  async function navigate(view: ViewName, bookName?: string) {
    prepare(view);
    await pushView(view, bookName ?? library.localBook.value?.name);
  }

  /**
   * 离开阅读页：在线阅读回进入时记下的来源，本地阅读回本地作品详情。
   *
   * 三个阅读页此前各写了一份相同的判断，现由返回表统一调用。
   */
  function leaveReader() {
    if (readerSource.value === "online") backFromOnlineReading();
    else void navigate("libraryDetail");
  }

  /**
   * 路由返回表：键是当前层，值是按一次返回要执行的动作。
   *
   * 与浮层栈共用唯一的 `back()` 入口：浮层先出栈，没有浮层才执行这里的动作。
   * 评论区是表中的普通一行；新增一层只加一行，不必改返回键、Esc 或页面按钮。
   * 根层的动作为空——停在根页时不响应返回。
   */
  const BACK_ACTIONS: Record<RouteLayer, () => void> = {
    discover: () => {},
    bookshelf: () => {},
    library: () => {},
    comments: () =>
      void navigate(remote.remoteBook.value ? "remoteDetail" : "discover"),
    remoteDetail: () => remote.closeRemoteBookDetail(),
    libraryDetail: () => void navigate("library"),
    reader: leaveReader,
    comicReader: leaveReader,
    audioPlayer: leaveReader,
  };

  /**
   * 唯一返回入口：Android 返回键、页面返回按钮都只调用它，Esc 只调用它的浮层部分。
   *
   * 一次返回恰好弹出一层。层级不再散落在 `ViewName`、`returnViews`、query 和各页面
   * 本地 ref 里：浮层由 `backLayers` 按打开顺序维护，路由层由 `BACK_ACTIONS` 声明。
   */
  function back() {
    if (dismissTopLayer()) return;
    BACK_ACTIONS[currentLayer()]();
  }

  function startOnlineReading(
    novel: Novel,
    mode: MediaType,
    chapterId?: number,
  ) {
    if (!isReaderView(currentView())) returnViews.online.value = currentView();
    online.prepareOnlineReading(novel, mode, chapterId);
    readerSource.value = "online";
    void pushView(
      mode === "audio"
        ? "audioPlayer"
        : mode === "comic"
          ? "comicReader"
          : "reader",
    );
  }

  /** 从本地书库直接打开在线版本。 */
  function startOnlineFromLibrary(mode: MediaType) {
    const book = library.localBook.value;
    const work = book?.[mode];
    if (!work?.id) return;
    startOnlineReading(
      {
        ...novelFromLocalWork(work, mode, work.lastUpdateTime || ""),
        bookshelfType: mode,
        sourcePath: work.onlinePath,
      },
      mode,
    );
  }

  /**
   * 侧栏应高亮的入口。
   *
   * 详情页与阅读页没有独立入口，归属到进入它的那一栏：远端详情的归属由进入时
   * 记下的返回视图决定，本地详情固定归书库。
   */
  function activeEntry(): ViewName {
    const view = currentView();
    if (view === "remoteDetail")
      return returnViews.remoteDetail.value === "discover"
        ? "discover"
        : "bookshelf";
    if (view === "libraryDetail") return "library";
    if (view === "bookshelf" || view === "library") return view;
    return "discover";
  }

  /** 打开远端评论页；`commentId` 用于定位单条评论。 */
  function openComments(tab: string, commentId?: number) {
    void pushComments({
      tab,
      ...(commentId == null ? {} : { comment: String(commentId) }),
    });
  }

  /** 在线阅读结束时的返回目标由进入时记录，避免写死来源页。 */
  function backFromOnlineReading() {
    void pushView(returnViews.online.value);
  }

  /** 对已下载内容追加下载：用本地目录信息重建远端标识。 */
  function continueDownload(mode: MediaType) {
    const work = library.localBook.value?.[mode];
    if (!work) return;
    void picker.openChapterPicker(novelFromLocalWork(work, mode), mode);
  }

  return {
    readerSource,
    currentView,
    activeEntry,
    navigate,
    startOnlineReading,
    startOnlineFromLibrary,
    back,
    continueDownload,
    openComments,
  };
}

export type WorkspaceDesk = ReturnType<typeof createWorkspaceDesk>;
