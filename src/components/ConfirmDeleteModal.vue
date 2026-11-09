<script setup lang="ts">
import { Trash2, X } from "@lucide/vue";
import type { Book } from "../types";
import ModalShell from "./ModalShell.vue";

defineProps<{ book?: Book }>();
const emit = defineEmits<{ close: []; confirm: [] }>();
</script>

<template>
  <ModalShell :open="Boolean(book)" @close="emit('close')">
    <section class="modal glass confirm-modal">
      <button class="close-button" title="关闭" @click="emit('close')">
        <X :size="20" /></button
      ><span class="modal-icon danger-icon"><Trash2 :size="22" /></span>
      <h2>删除本地内容？</h2>
      <p>《{{ book?.name }}》的本地文件将被删除，此操作无法撤销。</p>
      <div class="confirm-actions">
        <button class="text-button" @click="emit('close')">取消</button
        ><button class="danger-button" @click="emit('confirm')">
          确认删除
        </button>
      </div>
    </section>
  </ModalShell>
</template>

<style scoped>
.modal {
  position: relative;
  width: min(410px, 100%);
  max-height: calc(100dvh - 36px);
  padding: 28px;
  overflow-y: auto;
  border-radius: 20px;
}
.confirm-modal {
  width: min(420px, 100%);
}
.modal h2 {
  margin: 15px 0 5px;
  color: #69453d;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 23px;
}
.modal p {
  margin: 0 0 18px;
  color: #99766c;
  font-size: 12px;
  line-height: 1.7;
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
.danger-icon {
  color: #b85f51;
  background: #f7d6d0;
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
  transition: 0.2s;
}
.close-button:hover {
  color: var(--theme-color-dark);
  background: #fff0e7;
}
.confirm-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
}
.text-button {
  display: flex;
  align-items: center;
  padding: 8px 10px;
  border: 0;
  color: var(--theme-color-dark);
  background: none;
  font-weight: 600;
}
.danger-button {
  min-height: 36px;
  padding: 0 13px;
  border: 0;
  border-radius: 9px;
  color: #fff;
  background: #b85f51;
  font-size: 12px;
  font-weight: 600;
}
.danger-button:hover {
  background: #8d3127;
}
</style>
