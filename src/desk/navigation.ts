import { ref } from "vue";
import { router } from "../router";
import type { ViewName } from "../types";

/**
 * 视图与路由的唯一映射。
 *
 * 导航只有一份真相——`route.path`；这里的常量负责把视图名翻译成路径，不再回写
 * 任何「当前视图」状态。返回视图的记忆（`returnViews`）是模块级单例：它跨路由
 * 存活、但不驱动渲染，放在模块作用域比挂进 provider 更贴近它的生命周期。
 */

/** 评论区挂在作品详情路径下，但返回链上是独立一层。 */
export const COMMENTS_PATH = "/remote-book/comments";

/**
 * 返回链上的层。
 *
 * `ViewName` 是「侧栏与会话归属」用的粗粒度视图；返回需要更细的粒度，因此评论
 * 区单独参与路由层判定，但不进入侧栏枚举。新增一层只需在这里加一个成员，并在
 * `workspace` 的返回表里补一行。
 */
export type RouteLayer = ViewName | "comments";

/** 视图与路径的唯一映射；`libraryDetail` 的地址带书名，单独按前缀识别。 */
const VIEW_PATHS: Record<Exclude<ViewName, "libraryDetail">, string> = {
  discover: "/discover",
  bookshelf: "/bookshelf",
  remoteDetail: "/remote-book",
  library: "/library",
  reader: "/reader",
  comicReader: "/comic-reader",
  audioPlayer: "/audio",
};

const READER_VIEWS: readonly ViewName[] = [
  "reader",
  "comicReader",
  "audioPlayer",
];

/** 阅读类视图共享同一套返回与来源规则。 */
export function isReaderView(view: ViewName) {
  return READER_VIEWS.includes(view);
}

/** 视图对应的路径；`libraryDetail` 需要书名才能拼出真实地址。 */
function viewPath(view: ViewName, bookName?: string) {
  if (view === "libraryDetail") {
    return bookName ? `/library/${encodeURIComponent(bookName)}` : "/library";
  }
  return VIEW_PATHS[view];
}

/** 子路径也算该视图；评论区因此仍归属远端详情，侧栏高亮与会话归属不变。 */
function belongsTo(path: string, base: string) {
  return path === base || path.startsWith(`${base}/`);
}

/**
 * 由当前路径反推视图。
 *
 * 反查由上表推导，不另写一份 `if` 链——否则新增视图要改两处，漏改只会表现为
 * 侧栏高亮错位。未知路径按发现页处理。
 */
export function currentView(): ViewName {
  const { path } = router.currentRoute.value;
  if (path.startsWith("/library/")) return "libraryDetail";
  const entry = (Object.entries(VIEW_PATHS) as [ViewName, string][]).find(
    ([, base]) => belongsTo(path, base),
  );
  return entry ? entry[0] : "discover";
}

/**
 * 当前返回层。
 *
 * 与 `currentView()` 只在评论区有分歧：后者按前缀把它归入远端详情（侧栏与会话
 * 需要），前者把它当独立一层（返回手势需要）。其余层两者一致。
 */
export function currentLayer(): RouteLayer {
  if (router.currentRoute.value.path === COMMENTS_PATH) return "comments";
  return currentView();
}

type CommentsQuery = { tab: string; comment?: string };

/** 进入评论区；tab 与定位评论通过 query 传递。 */
export async function pushComments(query: CommentsQuery) {
  if (
    router.currentRoute.value.path === COMMENTS_PATH &&
    router.currentRoute.value.query.tab === query.tab &&
    router.currentRoute.value.query.comment === query.comment
  ) {
    return;
  }
  await router.push({ path: COMMENTS_PATH, query });
}

/** 评论区内部换层（切 tab、开关讨论详情）用 replace，不增长浏览器历史。 */
export async function setCommentsQuery(query: CommentsQuery) {
  await router.replace({ path: COMMENTS_PATH, query });
}

/** 跳转到目标视图；已在目标路径时只保留状态变更，不重复导航。 */
export async function pushView(view: ViewName, bookName?: string) {
  const target = viewPath(view, bookName);
  if (router.currentRoute.value.path === target) return;
  await router.push(target);
}

/** 各详情会话的返回目标，进入时写入、返回时读取。 */
export const returnViews = {
  remoteDetail: ref<ViewName>("bookshelf"),
  online: ref<ViewName>("remoteDetail"),
};
