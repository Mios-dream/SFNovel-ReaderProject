<script setup lang="ts">
import {
  CircleAlert,
  Download,
  ExternalLink,
  LoaderCircle,
  X,
} from "@lucide/vue";
import { computed } from "vue";
import type { UpdateInfo, UpdateStatus } from "../composables/useUpdater";

const props = defineProps<{
  open: boolean;
  status: UpdateStatus;
  info?: UpdateInfo;
  currentVersion: string;
  error: string;
}>();
const emit = defineEmits<{
  close: [];
  retry: [];
  openReleases: [];
}>();

const title = computed(() => {
  switch (props.status) {
    case "checking":
      return "正在检查更新";
    case "available":
      return "发现新版本";
    case "error":
      return "检查更新失败";
    default:
      return "应用更新";
  }
});

const versionLabel = computed(() =>
  props.currentVersion === "未知" ? "未知" : `v${props.currentVersion}`,
);
</script>

<template>
  <div v-if="open" class="update-backdrop" @click.self="emit('close')">
    <section class="update-modal glass" aria-labelledby="update-title">
      <button class="close-button" title="关闭" @click="emit('close')">
        <X :size="20" />
      </button>

      <span class="update-icon" :class="{ error: status === 'error' }">
        <CircleAlert v-if="status === 'error'" :size="24" />
        <LoaderCircle
          v-else-if="status === 'checking'"
          class="spin"
          :size="24"
        />
        <Download v-else :size="24" />
      </span>

      <h2 id="update-title">{{ title }}</h2>

      <p v-if="info && status === 'available'" class="version-line">
        {{ versionLabel }} <span>→</span> v{{ info.version }}
      </p>

      <template v-if="status === 'checking'">
        <p class="status-copy">正在从 GitHub 获取最新版本</p>
      </template>

      <template v-else-if="status === 'available'">
        <div v-if="info?.notes" class="release-notes">{{ info.notes }}</div>
        <div class="actions">
          <button class="text-button" @click="emit('close')">稍后</button>
          <button class="primary-button" @click="emit('openReleases')">
            <ExternalLink :size="16" />前往下载
          </button>
        </div>
      </template>

      <template v-else-if="status === 'error'">
        <p class="status-copy error-copy">{{ error }}</p>
        <div class="actions">
          <button class="secondary-button" @click="emit('retry')">
            重试检查
          </button>
          <button class="primary-button" @click="emit('openReleases')">
            <ExternalLink :size="16" />打开 Releases
          </button>
        </div>
      </template>
    </section>
  </div>
</template>

<style scoped>
.update-backdrop {
  position: fixed;
  inset: 0;
  z-index: 30;
  display: grid;
  padding: 18px;
  background: rgba(91, 49, 39, 0.24);
  place-items: center;
  animation: update-fade 180ms ease-out;
}
.update-modal {
  position: relative;
  width: min(430px, 100%);
  padding: 28px;
  border-radius: 22px;
  background: rgba(255, 249, 245, 0.94);
  animation: update-pop 280ms linear(0, 0.86 55%, 1.04 78%, 1);
}
.close-button {
  position: absolute;
  top: 16px;
  right: 16px;
  display: grid;
  width: 38px;
  height: 38px;
  border: 0;
  border-radius: 12px;
  color: #a17367;
  background: rgba(255, 255, 255, 0.7);
  place-items: center;
  transition:
    color 150ms ease-out,
    background 150ms ease-out;
}
.close-button:hover {
  color: var(--theme-color-dark);
  background: #fff0e7;
}
.update-icon {
  display: grid;
  width: 44px;
  height: 44px;
  border-radius: 14px;
  color: var(--theme-color-dark);
  background: #ffe5d5;
  place-items: center;
}
.update-icon.error {
  color: #b85f51;
  background: #f7d6d0;
}
h2 {
  margin: 16px 42px 0 0;
  color: #69453d;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 23px;
  line-height: 1.25;
}
.version-line {
  margin: 7px 0 0;
  color: #987166;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.version-line span {
  margin: 0 4px;
  color: #c29a8d;
}
.status-copy {
  margin: 16px 0 0;
  color: #80645d;
  font-size: 13px;
  line-height: 1.65;
}
.error-copy {
  color: #a35e52;
}
.release-notes {
  max-height: 180px;
  margin-top: 18px;
  overflow-y: auto;
  padding: 14px 15px;
  border: 1px solid #efd7ca;
  border-radius: 14px;
  color: #80645d;
  background: rgba(255, 255, 255, 0.5);
  font-size: 12px;
  line-height: 1.75;
  white-space: pre-line;
}
.actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 9px;
  margin-top: 24px;
}
.primary-button,
.secondary-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  min-height: 40px;
  border-radius: 11px;
  padding: 0 14px;
  font-size: 13px;
  font-weight: 600;
  transition:
    background 150ms ease-out,
    border-color 150ms ease-out,
    transform 150ms ease-out;
}
.primary-button {
  border: 0;
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 7px 14px rgba(226, 148, 100, 0.24);
}
.primary-button:hover {
  background: var(--theme-color-dark);
  transform: translateY(-1px);
}
.secondary-button {
  border: 1px solid #e7bda9;
  color: #8d5948;
  background: #fffaf7;
}
.secondary-button:hover {
  border-color: var(--theme-color);
  color: var(--theme-color-dark);
  background: #fff0e7;
}
.text-button {
  border: 0;
  padding: 8px 9px;
  color: #8d5948;
  background: none;
  font-size: 13px;
  font-weight: 600;
}
.text-button:hover {
  color: var(--theme-color-dark);
}
@keyframes update-fade {
  from {
    opacity: 0;
  }
}
@keyframes update-pop {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.98);
  }
}
@media (max-width: 480px) {
  .update-backdrop {
    align-items: end;
    padding: 0;
  }
  .update-modal {
    width: 100%;
    padding: 24px 20px calc(20px + env(safe-area-inset-bottom, 0px));
    border-right: 0;
    border-bottom: 0;
    border-left: 0;
    border-radius: 22px 22px 0 0;
  }
  .actions {
    flex-wrap: wrap;
  }
}
@media (prefers-reduced-motion: reduce) {
  .update-backdrop,
  .update-modal {
    animation: none;
  }
}
</style>
