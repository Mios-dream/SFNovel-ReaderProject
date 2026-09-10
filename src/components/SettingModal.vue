<script setup lang="ts">
import {
  LoaderCircle,
  RefreshCw,
  RotateCcw,
  Save,
  Settings2,
  X,
} from "lucide-vue-next";
import { reactive, ref, watch } from "vue";
import type { RequestPolicy } from "../types";

const props = defineProps<{
  open: boolean;
  policy: RequestPolicy;
  saving: boolean;
  dictionarySize: number;
  dictionaryUpdating: boolean;
  dictionaryChapterId: number;
}>();
const emit = defineEmits<{
  close: [];
  save: [policy: RequestPolicy];
  updateDictionary: [chapterId: number];
}>();

const defaultPolicy: RequestPolicy = {
  requestIntervalMs: 500,
  maxConcurrentDownloads: 1,
  appApiPreferredEnabled: true,
  androidDeviceReportEnabled: true,
};
// 使用本地草稿编辑，只有提交表单时才将设置发送给后端。
const draft = reactive<RequestPolicy>({ ...props.policy });
const dictionaryChapterDraft = ref(props.dictionaryChapterId);

watch(
  () => props.policy,
  (policy) => Object.assign(draft, policy),
  { deep: true, immediate: true },
);
watch(
  () => props.dictionaryChapterId,
  (chapterId) => {
    dictionaryChapterDraft.value = chapterId;
  },
  { immediate: true },
);

/**
 * Restores the editable request policy draft to conservative defaults.
 *
 * @returns No value; mutates only the local draft until the user submits it.
 */
function resetDraft() {
  Object.assign(draft, defaultPolicy);
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

      <form class="settings-form" @submit.prevent="emit('save', { ...draft })">
        <div class="settings-content">
          <section class="settings-group">
            <header class="settings-group-header">
              <strong>下载设置</strong>
            </header>

            <div class="setting-row">
              <div class="setting-info">
                <strong>下载请求间隔</strong>
                <small>每次请求之间等待的时间，单位为毫秒</small>
              </div>
              <label class="setting-control">
                <input
                  v-model.number="draft.requestIntervalMs"
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
                  v-model.number="draft.maxConcurrentDownloads"
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
                <strong>优先使用 App API 下载正文</strong>
                <small
                  >存在 App 凭证时优先使用 App API。网页图片正文需要
                  OCR，较耗性能且可能存在识别错误</small
                >
              </div>
              <label class="setting-control toggle-control">
                <input v-model="draft.appApiPreferredEnabled" type="checkbox" />
                <span class="toggle" aria-hidden="true"></span>
                <span>{{
                  draft.appApiPreferredEnabled ? "已启用" : "已关闭"
                }}</span>
              </label>
            </div>
          </section>
          <section class="settings-group dictionary-group">
            <header class="settings-group-header">
              <strong>安全</strong>
            </header>
            <div class="setting-row">
              <div class="setting-info">
                <strong>App 设备信息上报（实验）</strong>
                <small
                  >App
                  登录成功后尝试上报当前安装信息，可能会减少账号风控风险；默认开启</small
                >
              </div>
              <label class="setting-control toggle-control">
                <input
                  v-model="draft.androidDeviceReportEnabled"
                  type="checkbox"
                />
                <span class="toggle" aria-hidden="true"></span>
                <span>{{
                  draft.androidDeviceReportEnabled ? "已启用" : "已关闭"
                }}</span>
              </label>
            </div>
          </section>

          <section class="settings-group dictionary-group">
            <header class="settings-group-header">
              <strong>解码字典</strong>
              <small>从网页小说章节构建解码字典</small>
            </header>

            <div class="setting-row">
              <div class="setting-info">
                <strong>公开章节id</strong>
                <small>用于构建解码字典使用的网页小说章节id</small>
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
        </div>

        <footer class="settings-actions">
          <button class="reset-button" type="button" @click="resetDraft">
            <RotateCcw :size="16" />重设设置
          </button>
          <button class="primary-button" :disabled="saving" type="submit">
            <LoaderCircle v-if="saving" class="spin" :size="17" />
            <Save v-else :size="17" />
            {{ saving ? "正在保存" : "保存设置" }}
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
  margin: 5%;
}
.settings-modal {
  width: min(760px, 100%);
  height: min(680px, 80%);
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
  min-height: 0;
  overflow-y: auto;
  padding-right: 8px;
}
.settings-group + .settings-group {
  margin-top: 26px;
}
.settings-group-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 16px;
  border-bottom: 3px solid #efd7ca;
  padding-bottom: 10px;
}
.settings-group-header strong {
  color: #805245;
  font-size: 19px;
  font-weight: bold;
}
.settings-group-header small {
  color: #b08072;
  font-size: 11px;
}
.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
  min-height: 76px;
  border-bottom: 1px solid #efd7ca;
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
  margin-top: 5px;
  color: #a17367;
  font-size: 12px;
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
input {
  width: 130px;
  min-height: 42px;
  border: 1px solid #efcfbf;
  border-radius: 10px;
  padding: 0 11px;
  color: #624740;
  background: #fffaf7;
  outline: none;
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
  margin-top: 28px;
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
  .settings-modal {
    height: 80%;
    padding: 22px;
  }
  .setting-row {
    flex-direction: column;
    align-items: stretch;
    gap: 12px;
    padding: 17px 0;
  }
  .settings-group-header {
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
  }
  .setting-control {
    flex: 1 1 auto;
    width: 100%;
    justify-content: flex-start;
  }
  .secondary-button {
    align-self: flex-start;
  }
  .settings-actions {
    margin-top: 22px;
  }
}
</style>
