<script setup lang="ts">
import { computed } from "vue";
import { CloudDownload } from "@lucide/vue";

import AppSidebar from "./AppSidebar.vue";
import type { ViewName } from "../types";

import { useAuth, useBookshelf, useJobs, useLibrary, useSettings, useWorkspace } from "../desk";

const auth = useAuth();
const bookshelf = useBookshelf();
const jobs = useJobs();
const library = useLibrary();
const settings = useSettings();
const workspace = useWorkspace();

const active = computed<ViewName>(() => workspace.activeEntry());

function navigate(view: ViewName) {
  void workspace.navigate(view);
}
</script>

<template>
  <div class="page-frame">
    <AppSidebar
      :active="active"
      :bookshelf-count="bookshelf.bookshelf.value.length"
      :library-count="library.books.value.length"
      :auth="auth.auth.value"
      :profile="auth.accountProfile.value"
      @navigate="navigate"
      @account="auth.openAccount"
      @settings="settings.openSettings"
    />
    <slot />
    <button
      class="queue-fab"
      title="打开下载列表"
      @click="jobs.queueOpen.value = true"
    >
      <CloudDownload :size="22" />
      <span v-if="jobs.runningJobs.value.length" class="queue-count">
        {{ jobs.runningJobs.value.length }}
      </span>
    </button>
  </div>
</template>

<style scoped>
.page-frame {
  display: grid;
  grid-template-columns: 238px minmax(0, 1fr);
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
