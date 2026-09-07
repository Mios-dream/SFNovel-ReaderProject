<template>
  <RouterView />
  <AppOverlays />
</template>
<script setup lang="ts">
import { provide, reactive, watch } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import AppOverlays from "./components/AppOverlays.vue";
import { useNovelDesk } from "./composables/useNovelDesk";
import { deskInjectionKey } from "./deskContext";

const deskState = useNovelDesk();
const desk = reactive(deskState);
const router = useRouter();
const route = useRoute();
provide(deskInjectionKey, deskState);
watch(
  () => desk.active,
  (view) => {
    if (view === "reader" && route.path !== "/reader")
      void router.push("/reader");
    if (view === "comicReader" && route.path !== "/comic-reader")
      void router.push("/comic-reader");
    if (view === "audioPlayer" && route.path !== "/audio")
      void router.push("/audio");
    if (
      view === "libraryDetail" &&
      desk.localBook &&
      !route.path.startsWith("/library/")
    ) {
      void router.push(`/library/${encodeURIComponent(desk.localBook.name)}`);
    }
    if (view === "remoteDetail" && route.path !== "/remote-book")
      void router.push("/remote-book");
    if (
      (view === "discover" || view === "bookshelf" || view === "library") &&
      (route.path.startsWith("/library/") ||
        route.path === "/reader" ||
        route.path === "/comic-reader" ||
        route.path === "/audio")
    ) {
      void router.push(`/${view}`);
    }
    if (view === "bookshelf" && route.path === "/remote-book")
      void router.push("/bookshelf");
  },
);

watch(
  () => route.path,
  (path) => {
    let view: "discover" | "bookshelf" | "library" | undefined;
    switch (path) {
      case "/discover":
        view = "discover";
        break;
      case "/bookshelf":
        view = "bookshelf";
        break;
      case "/library":
        view = "library";
        break;
      default:
        view = undefined;
        break;
    }
    if (view && desk.active !== view) desk.navigate(view);
  },
  { immediate: true },
);
</script>
