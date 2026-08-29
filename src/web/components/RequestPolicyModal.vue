<script setup lang="ts">
import { LoaderCircle, Save, Settings2, X } from "lucide-vue-next";
import { reactive, watch } from "vue";
import type { RequestPolicy } from "../types";

const props = defineProps<{
  open: boolean;
  policy: RequestPolicy;
  saving: boolean;
}>();
const emit = defineEmits<{ close: []; save: [policy: RequestPolicy] }>();
const draft = reactive<RequestPolicy>({ ...props.policy });

watch(
  () => props.policy,
  (policy) => Object.assign(draft, policy),
  { deep: true, immediate: true },
);
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <section class="modal glass">
      <button
        class="close-button"
        title="关闭"
        :disabled="saving"
        @click="emit('close')"
      >
        <X :size="20" />
      </button>
      <span class="modal-icon"><Settings2 :size="22" /></span>
      <h2>请求设置</h2>
      <form @submit.prevent="emit('save', { ...draft })">
        <label>
          <span>下载请求间隔（毫秒）</span>
          <input
            v-model.number="draft.requestIntervalMs"
            type="number"
            min="250"
            max="10000"
            step="50"
            required
          />
        </label>
        <label>
          <span>并发下载任务</span>
          <input
            v-model.number="draft.maxConcurrentDownloads"
            type="number"
            min="1"
            max="3"
            step="1"
            required
          />
        </label>
        <button class="primary-button full" :disabled="saving" type="submit">
          <LoaderCircle v-if="saving" class="spin" :size="17" /><Save
            v-else
            :size="17"
          />{{ saving ? "正在保存" : "保存设置" }}
        </button>
      </form>
    </section>
  </div>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 10;
  display: grid;
  padding: 18px;
  background: rgba(91, 49, 39, 0.24);
  place-items: center;
}
.modal {
  position: relative;
  width: min(410px, 100%);
  padding: 28px;
  border-radius: 20px;
  background: rgba(255, 249, 245, 0.8);
}
.modal h2 {
  margin: 15px 0 20px;
  color: #69453d;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 23px;
}
.modal-icon {
  display: grid;
  width: 42px;
  height: 42px;
  border-radius: 13px;
  color: var(--theme-color-dark);
  background: #ffe5d5;
  place-items: center;
}
.close-button {
  position: absolute;
  top: 16px;
  right: 16px;
  display: grid;
  width: 40px;
  height: 40px;
  border: 0;
  border-radius: 13px;
  color: #a17367;
  background: rgba(255, 255, 255, 0.7);
  place-items: center;
}
.close-button:hover {
  color: var(--theme-color-dark);
  background: #fff0e7;
}
form {
  display: grid;
  gap: 15px;
}
label {
  display: grid;
  gap: 7px;
  color: #87645a;
  font-size: 13px;
}
input {
  width: 100%;
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
.primary-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 42px;
  padding: 0 16px;
  border: 0;
  border-radius: 10px;
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 7px 12px #e2946440;
  font-size: 13px;
  font-weight: 600;
}
.primary-button:hover {
  background: var(--theme-color-dark);
}
.primary-button:disabled,
.close-button:disabled {
  cursor: wait;
  opacity: 0.7;
}
.full {
  width: 100%;
  margin-top: 5px;
}
</style>
