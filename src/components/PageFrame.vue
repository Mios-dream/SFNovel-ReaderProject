<script setup lang="ts">
import { computed, inject } from "vue";
import { CloudDownload } from "lucide-vue-next";
import { useRoute, useRouter } from "vue-router";
import AppSidebar from "./AppSidebar.vue";
import { deskInjectionKey } from "../deskContext";
import type { ViewName } from "../types";

function requireDesk() {
  const desk = inject(deskInjectionKey);
  if (!desk) throw new Error("Desk context is unavailable");
  return desk;
}

const desk = requireDesk();
const route = useRoute();
const router = useRouter();
const active = computed<ViewName>(() => {
  if (route.path === "/bookshelf") return "bookshelf";
  if (route.path === "/library") return "library";
  return "discover";
});

async function navigate(view: ViewName) {
  desk.navigate(view);
  await router.push(`/${view}`);
}
</script>

<template>
  <div class="page-frame">
    <AppSidebar
      :active="active"
      :bookshelf-count="desk.bookshelf.value.length"
      :library-count="desk.library.value.length"
      :auth="desk.auth.value"
      :profile="desk.accountProfile.value"
      @navigate="navigate"
      @account="desk.openAccount"
      @request-settings="desk.openRequestPolicy"
    />
    <slot />
    <button
      class="queue-fab"
      title="打开下载列表"
      @click="desk.queueOpen.value = true"
    >
      <CloudDownload :size="22" />
      <span v-if="desk.runningJobs.value.length" class="queue-count">
        {{ desk.runningJobs.value.length }}
      </span>
    </button>
  </div>
</template>

<style scoped>
.page-frame {
  display: grid;
  grid-template-columns: 238px minmax(0, 1fr);
  gap: 18px;
  height: 100dvh;
  min-height: 0;
  padding: 18px;
  overflow: hidden;
  background-color: var(--background-color);
}
.queue-fab {
  position: fixed;
  right: 28px;
  bottom: 28px;
  z-index: 8;
  display: grid;
  width: 56px;
  height: 56px;
  border: 0;
  border-radius: 18px;
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 12px 28px #a85d4250;
  place-items: center;
}
.queue-fab:hover {
  background: var(--theme-color-dark);
  transform: translateY(-2px);
}
.queue-count {
  position: absolute;
  top: -6px;
  right: -5px;
  display: grid;
  width: 27px;
  height: 27px;
  border-radius: 9px;
  color: #a85d42;
  background: #ffe0ce;
  font: 12px monospace;
  place-items: center;
}
@media (max-width: 1150px) {
  .page-frame {
    grid-template-columns: 215px minmax(0, 1fr);
  }
}
@media (max-width: 760px) {
  .page-frame {
    display: block;
    height: auto;
    min-height: 100dvh;
    padding: calc(8px + env(safe-area-inset-top)) 12px
      calc(78px + env(safe-area-inset-bottom));
    overflow: visible;
  }
  .queue-fab {
    right: 18px;
    bottom: calc(70px + env(safe-area-inset-bottom));
    width: 52px;
    height: 52px;
    border-radius: 16px;
  }
}
</style>
