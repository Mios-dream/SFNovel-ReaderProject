/**
 * 分页阅读器模块出口。
 *
 * 对外暴露类型、分页/渲染纯函数，以及 Vue 组合式函数，
 * 组件只需从这里导入，避免深入内部文件结构。
 */

export { usePagedReader } from "./usePagedReader";
export type {
  UsePagedReaderOptions,
  PagedLayoutOptions,
  NeighborChapterContent,
} from "./usePagedReader";
export { PagedEngine } from "./pagedEngine";
export type { PagedEngineCallbacks } from "./pagedEngine";
export { paginateText } from "./paginate";
export { renderPage } from "./renderPage";
export { wrapLine, wrapParagraph, measureContext } from "./textLayout";
export type {
  ReaderChapterPart,
  ReaderPage,
  ReaderPageItem,
  ReaderStyle,
  ReaderViewport,
} from "./types";
