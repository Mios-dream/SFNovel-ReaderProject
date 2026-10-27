import { createRouter, createWebHashHistory } from "vue-router";
import AudioPlayerPage from "./pages/AudioPlayerPage.vue";
import BookshelfPage from "./pages/BookshelfPage.vue";
import ComicReaderPage from "./pages/ComicReaderPage.vue";
import DiscoverPage from "./pages/DiscoverPage.vue";
import LibraryPage from "./pages/LibraryPage.vue";
import LocalBookDetailPage from "./pages/LocalBookDetailPage.vue";
import ReaderPage from "./pages/ReaderPage.vue";
import RemoteBookDetailPage from "./pages/RemoteBookDetailPage.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", redirect: "/discover" },
    { path: "/discover", component: DiscoverPage },
    { path: "/bookshelf", component: BookshelfPage },
    { path: "/remote-book", component: RemoteBookDetailPage },
    { path: "/library", component: LibraryPage },
    { path: "/library/:name", component: LocalBookDetailPage },
    { path: "/reader", component: ReaderPage },
    { path: "/comic-reader", component: ComicReaderPage },
    { path: "/audio", component: AudioPlayerPage },
  ],
});
