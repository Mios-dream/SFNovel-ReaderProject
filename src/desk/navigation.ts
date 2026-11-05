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

/** 子路径也算该视图：`/remote-book/comments` 仍属于远端详情。 */
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

/** 评论区不在主视图枚举内，tab 与定位评论通过 query 传递。 */
export async function pushComments(query: {
  tab: string;
  comment?: string;
}) {
  const path = "/remote-book/comments";
  if (
    router.currentRoute.value.path === path &&
    router.currentRoute.value.query.tab === query.tab &&
    router.currentRoute.value.query.comment === query.comment
  ) {
    return;
  }
  await router.push({ path, query });
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
