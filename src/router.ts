import { createRouter, createWebHashHistory } from "vue-router";
import BookshelfPage from "./pages/BookshelfPage.vue";
import DiscoverPage from "./pages/DiscoverPage.vue";
import LibraryPage from "./pages/LibraryPage.vue";
import LocalAudioPlayerPage from "./pages/LocalAudioPlayerPage.vue";
import LocalBookDetailPage from "./pages/LocalBookDetailPage.vue";
import LocalComicReaderPage from "./pages/LocalComicReaderPage.vue";
import LocalReaderPage from "./pages/LocalReaderPage.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", redirect: "/discover" },
    { path: "/discover", component: DiscoverPage },
    { path: "/bookshelf", component: BookshelfPage },
    { path: "/library", component: LibraryPage },
    { path: "/library/:name", component: LocalBookDetailPage },
    { path: "/reader", component: LocalReaderPage },
    { path: "/comic-reader", component: LocalComicReaderPage },
    { path: "/audio", component: LocalAudioPlayerPage },
  ],
});
