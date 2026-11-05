<script setup lang="ts">
import {
  BookOpenText,
  Download,
  HardDrive,
  LoaderCircle,
  Monitor,
  RefreshCw,
  RotateCcw,
  Save,
  Settings2,
  X,
} from "@lucide/vue";
import { computed, reactive, ref, watch } from "vue";
import type { UpdateStatus } from "../desk";
import type { AppSettings } from "../types";
import SettingSelect from "./SettingSelect.vue";

const props = defineProps<{
  open: boolean;
  settings: AppSettings;
  saving: boolean;
  dictionarySize: number;
  dictionaryUpdating: boolean;
  dictionaryChapterId: number;
  currentVersion: string;
  updateStatus: UpdateStatus;
  updateVersion?: string;
}>();
const emit = defineEmits<{
  close: [];
  save: [settings: AppSettings];
  updateDictionary: [chapterId: number];
  checkUpdate: [];
  showUpdate: [];
  openReleases: [];
}>();

const defaultSettings: AppSettings = {
  download: {
    requestIntervalMs: 500,
    maxConcurrentDownloads: 1,
    ocrProcessFilesEnabled: false,
  },
  storage: {
    useExternalStorage: false,
  },
  desktop: {
    closeToTray: true,
    autostart: false,
  },
  signin: {
    autoSigninEnabled: false,
  },
  updates: {
    autoCheck: true,
  },
};

function cloneSettings(settings: AppSettings): AppSettings {
  return {
    download: { ...settings.download },
    storage: { ...settings.storage },
    desktop: { ...settings.desktop },
    signin: { ...settings.signin },
    updates: { ...settings.updates },
  };
}

// 使用本地草稿编辑，只有提交表单时才将设置发送给后端。
const draft = reactive<AppSettings>(cloneSettings(props.settings));
const dictionaryChapterDraft = ref(props.dictionaryChapterId);
const isAndroid = /Android/i.test(navigator.userAgent);
const isWindows = /Windows/i.test(navigator.userAgent);
const closeActions = [
  { label: "后台运行", value: true },
  { label: "直接退出", value: false },
];
const storageLocations = [
  { label: "内部存储", value: false },
  { label: "外部存储", value: true },
];

watch(
  () => props.settings,
  (settings) => Object.assign(draft, cloneSettings(settings)),
  { deep: true, immediate: true },
);
watch(
  () => props.dictionaryChapterId,
  (chapterId) => {
    dictionaryChapterDraft.value = chapterId;
  },
  { immediate: true },
);

function resetDraft() {
  Object.assign(draft, cloneSettings(defaultSettings));
}

function handleUpdateAction() {
  if (props.updateStatus === "available" || props.updateStatus === "error") {
    emit("showUpdate");
    return;
  }
  emit("checkUpdate");
}

function updateHint() {
  switch (props.updateStatus) {
    case "checking":
      return "正在连接 GitHub";
    case "available":
      return `发现新版本 v${props.updateVersion}`;
    case "error":
      return "检查失败，可重试或前往 GitHub";
    default:
      return "";
  }
}

const versionLabel = computed(() =>
  props.currentVersion === "未知" ? "未知" : `v${props.currentVersion}`,
);

function updateActionLabel() {
  switch (props.updateStatus) {
    case "checking":
      return "正在检查";
    case "available":
      return "查看更新";
    case "error":
      return "查看详情";
    default:
      return "检查更新";
  }
}
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <section class="modal glass settings-modal">
      <button
        class="close-button"
        title="关闭"
        :disabled="saving"
        @click="emit('close')"
      >
        <X :size="20" />
      </button>
      <header class="settings-header">
        <span class="modal-icon"><Settings2 :size="22" /></span>
        <h2>设置</h2>
      </header>

      <form
        class="settings-form"
        @submit.prevent="emit('save', cloneSettings(draft))"
      >
        <div class="settings-content">
          <section class="settings-group">
            <header class="settings-group-header">
              <span class="settings-group-icon"><Download :size="16" /></span>
              <strong>下载</strong>
            </header>

            <div class="setting-row">
              <div class="setting-info">
                <strong>下载请求间隔</strong>
                <small>每次请求之间等待的时间，单位为毫秒</small>
              </div>
              <label class="setting-control">
                <input
                  v-model.number="draft.download.requestIntervalMs"
                  type="number"
                  min="100"
                  max="10000"
                  step="50"
                  required
                />
              </label>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <strong>并发下载任务</strong>
                <small>同时处理的下载任务数量</small>
              </div>
              <label class="setting-control">
                <input
                  v-model.number="draft.download.maxConcurrentDownloads"
                  type="number"
                  min="1"
                  max="3"
                  step="1"
                  required
                />
              </label>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <strong>保存 OCR 过程文件</strong>
                <small>用于检查识别问题</small>
              </div>
              <label class="setting-control toggle-control">
                <input
                  v-model="draft.download.ocrProcessFilesEnabled"
                  type="checkbox"
                />
                <span class="toggle" aria-hidden="true"></span>
                <span>{{
                  draft.download.ocrProcessFilesEnabled ? "已启用" : "已关闭"
                }}</span>
              </label>
            </div>
          </section>
          <section class="settings-group dictionary-group">
            <header class="settings-group-header">
              <span class="settings-group-icon"
                ><BookOpenText :size="16"
              /></span>
              <strong>解码字典</strong>
              <small>从网页小说章节构建解码字典</small>
            </header>

            <div class="setting-row">
              <div class="setting-info">
                <strong>公开章节id</strong>
                <small>更新字典所需的小说章节id</small>
              </div>
              <label class="setting-control">
                <input
                  v-model.number="dictionaryChapterDraft"
                  type="number"
                  min="1"
                  required
                />
              </label>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <strong>构建字典</strong>
                <small>当前已收录 {{ dictionarySize }} 个字符映射</small>
              </div>
              <button
                class="secondary-button"
                :disabled="dictionaryUpdating"
                type="button"
                @click="emit('updateDictionary', dictionaryChapterDraft)"
              >
                <LoaderCircle
                  v-if="dictionaryUpdating"
                  class="spin"
                  :size="16"
                />
                <RefreshCw v-else :size="16" />
                {{ dictionaryUpdating ? "正在比对" : "更新字典" }}
              </button>
            </div>
          </section>
          <section v-if="!isAndroid" class="settings-group">
            <header class="settings-group-header">
              <span class="settings-group-icon"><Monitor :size="16" /></span>
              <strong>桌面端</strong>
            </header>

            <div class="setting-row">
              <div class="setting-info">
                <strong>关闭窗口时</strong>
                <small>{{
                  draft.desktop.closeToTray
                    ? "托盘图标可重新打开窗口"
                    : "关闭窗口将结束进程"
                }}</small>
              </div>
              <div class="setting-control">
                <SettingSelect
                  v-model="draft.desktop.closeToTray"
                  :options="closeActions"
                  label="关闭窗口时"
                />
              </div>
            </div>

            <div v-if="isWindows" class="setting-row">
              <div class="setting-info">
                <strong>开机自启</strong>
              </div>
              <label class="setting-control toggle-control">
                <input v-model="draft.desktop.autostart" type="checkbox" />
                <span class="toggle" aria-hidden="true"></span>
                <span>{{ draft.desktop.autostart ? "已启用" : "已关闭" }}</span>
              </label>
            </div>
          </section>
          <section v-if="isAndroid" class="settings-group">
            <header class="settings-group-header">
              <span class="settings-group-icon"><HardDrive :size="16" /></span>
              <strong>存储</strong>
            </header>

            <div class="setting-row">
              <div class="setting-info">
                <strong>本地存储位置</strong>
                <small>外部存储位于 Download/SF Novel Flow</small>
              </div>
              <div class="setting-control">
                <SettingSelect
                  v-model="draft.storage.useExternalStorage"
                  :options="storageLocations"
                  label="本地存储位置"
                />
              </div>
            </div>
          </section>
          <section class="settings-group update-group">
            <header class="settings-group-header">
              <span class="settings-group-icon"><RefreshCw :size="16" /></span>
              <strong>更新</strong>
            </header>

            <div class="setting-row">
              <div class="setting-info">
                <strong>当前版本</strong>
              </div>
              <span class="version-label">{{ versionLabel }}</span>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <strong>自动检查更新</strong>
                <small>每次启动时检查 GitHub 最新版本</small>
              </div>
              <label class="setting-control toggle-control">
                <input v-model="draft.updates.autoCheck" type="checkbox" />
                <span class="toggle" aria-hidden="true"></span>
                <span>{{ draft.updates.autoCheck ? "已启用" : "已关闭" }}</span>
              </label>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <strong>应用更新</strong>
                <small v-if="updateHint()">{{ updateHint() }}</small>
              </div>
              <button
                class="secondary-button"
                :disabled="updateStatus === 'checking'"
                type="button"
                @click="handleUpdateAction"
              >
                <LoaderCircle
                  v-if="updateStatus === 'checking'"
                  class="spin"
                  :size="16"
                />
                <Download v-else-if="updateStatus === 'available'" :size="16" />
                <RefreshCw v-else :size="16" />
                {{ updateActionLabel() }}
              </button>
            </div>
          </section>
        </div>

        <footer class="settings-actions">
          <button class="reset-button" type="button" @click="resetDraft">
            <RotateCcw :size="16" />重设
          </button>
          <button class="primary-button" :disabled="saving" type="submit">
            <LoaderCircle v-if="saving" class="spin" :size="17" />
            <Save v-else :size="17" />
            {{ saving ? "正在保存" : "保存" }}
          </button>
        </footer>
      </form>
    </section>
  </div>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 10;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 18px;
  background: rgba(91, 49, 39, 0.24);
}
.modal {
  position: relative;
  width: min(410px, 100%);
  padding: 28px;
  border-radius: 20px;
  background: rgba(255, 249, 245, 0.8);
  margin: 0;
}
.settings-modal {
  width: min(760px, 100%);
  height: min(680px, calc(100vh - 40px));
  height: min(680px, calc(100dvh - 40px));
  max-height: calc(100vh - 36px);
  max-height: calc(100dvh - 36px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.settings-header {
  display: flex;
  align-items: center;
  gap: 13px;
  padding-right: 48px;
}
.modal h2 {
  color: #69453d;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 23px;
  margin: 0;
}
.settings-header p {
  margin: 0;
  color: #99766c;
  font-size: 12px;
}
.modal-icon {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  width: 42px;
  height: 42px;
  border-radius: 13px;
  color: var(--theme-color-dark);
  background: #ffe5d5;
}
.close-button {
  position: absolute;
  top: 16px;
  right: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border: 0;
  border-radius: 13px;
  color: #a17367;
  background: rgba(255, 255, 255, 0.7);
}
.close-button:hover {
  color: var(--theme-color-dark);
  background: #fff0e7;
}
.close-button:disabled {
  cursor: wait;
  opacity: 0.7;
}
.settings-form {
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  min-height: 0;
  margin-top: 27px;
}
.settings-content {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-height: 0;
  overflow-y: auto;
  padding: 2px 8px 4px 0;
  overscroll-behavior: contain;
}
.settings-group {
  padding-bottom: 8px;
  border: 1px solid rgba(217, 173, 151, 0.32);
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.58);
  box-shadow:
    0 8px 24px rgba(137, 76, 55, 0.045),
    inset 0 1px 0 rgba(255, 255, 255, 0.72);
}
.settings-group-header {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 13px 16px 5px;
}
.settings-group-icon {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border-radius: 9px;
  color: var(--theme-color-dark);
  background: #ffeadf;
}
.settings-group-header strong {
  color: #69453d;
  font-size: 15px;
  font-weight: 700;
}
.settings-group-header small {
  min-width: 0;
  margin-left: auto;
  color: #a17367;
  font-size: 12px;
  line-height: 1.4;
  text-align: right;
}
.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
  min-height: 62px;
  padding: 10px 16px;
}
.setting-info {
  flex: 1 1 auto;
  min-width: 0;
}
.setting-info strong,
.setting-info small {
  display: block;
}
.setting-info strong {
  color: #69453d;
  font-size: 14px;
}
.setting-info small {
  margin-top: 4px;
  color: #a17367;
  font-size: 12px;
}
.update-group .setting-row {
  min-height: 58px;
}
.setting-control {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: flex-end;
  gap: 9px;
  color: #987166;
  font-size: 12px;
}
.version-label {
  color: #8d5948;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
input {
  width: 130px;
  min-height: 42px;
  border: 1px solid #efcfbf;
  border-radius: 10px;
  padding: 0 11px;
  color: #624740;
  background: #fffaf7;
  outline: none;
  font-size: 16px;
}
input:focus {
  border-color: var(--theme-color);
  box-shadow: 0 0 0 3px #f8d7c6;
}
input[type="checkbox"] {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
  pointer-events: none;
}
.toggle-control {
  position: relative;
  cursor: pointer;
}
.toggle {
  position: relative;
  display: inline-flex;
  width: 42px;
  height: 24px;
  border-radius: 99px;
  background: #dfc7bd;
  transition: background 0.2s;
}
.toggle::after {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: #fffaf7;
  box-shadow: 0 1px 4px rgba(91, 49, 39, 0.2);
  content: "";
  transition: transform 0.2s;
}
.toggle-control input:checked + .toggle {
  background: var(--theme-color);
}
.toggle-control input:checked + .toggle::after {
  transform: translateX(18px);
}
.primary-button,
.secondary-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 42px;
  border-radius: 10px;
  font-size: 13px;
  font-weight: 600;
  transition: 0.2s;
}
.primary-button {
  min-width: 126px;
  border: 0;
  padding: 0 16px;
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 7px 12px #e2946440;
}
.primary-button:hover {
  background: var(--theme-color-dark);
  transform: translateY(-1px);
}
.primary-button:disabled,
.secondary-button:disabled {
  cursor: wait;
  opacity: 0.7;
}
.secondary-button {
  flex: 0 0 auto;
  border: 1px solid #e7bda9;
  padding: 0 13px;
  color: #8d5948;
  background: #fffaf7;
}
.secondary-button:hover:not(:disabled) {
  border-color: var(--theme-color);
  color: var(--theme-color-dark);
  background: #fff0e7;
}
.settings-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 12px;
  padding-top: 4px;
}
.reset-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  min-height: 40px;
  border: 1px solid #efd0c0;
  border-radius: 10px;
  padding: 0 13px;
  color: #8d5948;
  background: #fffaf7;
  font-size: 12px;
}
.reset-button:hover {
  border-color: var(--theme-color);
  color: var(--theme-color-dark);
  background: #fff0e7;
}

@media (max-width: 600px) {
  .modal-backdrop {
    z-index: 20;
    align-items: flex-end;
    padding: 0;
  }
  .settings-modal {
    width: 100%;
    height: auto;
    max-height: calc(75dvh - env(safe-area-inset-top, 0px) - 10px);
    padding: 17px 16px calc(12px + env(safe-area-inset-bottom, 0px));
    border-right: 0;
    border-bottom: 0;
    border-left: 0;
    border-radius: 20px 20px 0 0;
    background: rgba(255, 249, 245, 0.97);
    box-shadow: 0 -12px 32px rgba(137, 76, 55, 0.16);
  }
  .settings-header {
    gap: 10px;
    padding-right: 42px;
  }
  .modal-icon {
    width: 36px;
    height: 36px;
    border-radius: 11px;
  }
  .modal-icon svg {
    width: 18px;
    height: 18px;
  }
  .modal h2 {
    font-size: 20px;
  }
  .close-button {
    top: 13px;
    right: 13px;
    width: 36px;
    height: 36px;
    border-radius: 11px;
  }
  .settings-form {
    margin-top: 16px;
  }
  .settings-content {
    gap: 12px;
    padding: 2px 2px 4px 0;
    scroll-padding-bottom: 8px;
  }
  .settings-group {
    padding-bottom: 6px;
    border-radius: 15px;
  }
  .settings-group-header {
    align-items: center;
    gap: 8px;
    padding: 11px 12px 3px;
  }
  .settings-group-icon {
    width: 26px;
    height: 26px;
    border-radius: 8px;
  }
  .settings-group-icon svg {
    width: 14px;
    height: 14px;
  }
  .settings-group-header strong {
    font-size: 14px;
  }
  .settings-group-header small {
    font-size: 12px;
    line-height: 1.4;
    text-align: right;
  }
  .setting-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px 12px;
    min-height: 0;
    padding: 11px 12px;
  }
  .update-group .setting-row {
    min-height: 0;
  }
  .setting-info strong {
    font-size: 13px;
    line-height: 1.35;
  }
  .setting-info small {
    margin-top: 2px;
    font-size: 12px;
    line-height: 1.45;
  }
  .setting-control {
    width: auto;
    gap: 7px;
    justify-content: flex-end;
    justify-self: end;
    font-size: 12px;
  }
  .setting-control input:not([type="checkbox"]) {
    width: 98px;
    min-height: 38px;
    border-radius: 9px;
    padding: 0 10px;
    font-size: 12px;
  }
  .toggle {
    width: 40px;
    height: 23px;
  }
  .toggle::after {
    width: 17px;
    height: 17px;
  }
  .toggle-control input:checked + .toggle::after {
    transform: translateX(17px);
  }
  .secondary-button {
    min-height: 36px;
    padding: 0 10px;
    font-size: 12px;
  }
  .settings-actions {
    gap: 10px;
    margin-top: 13px;
    padding-top: 12px;
  }
  .reset-button {
    flex: 0 0 auto;
    min-width: 76px;
    min-height: 40px;
    padding: 0 14px;
  }
  .primary-button {
    flex: 0 0 auto;
    min-width: 96px;
    min-height: 40px;
  }
}
</style>
