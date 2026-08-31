<script setup lang="ts">
import {
  Download,
  FileArchive,
  FileDown,
  FileText,
  Headphones,
  X,
} from "lucide-vue-next";

defineProps<{
  open: boolean;
  textChapterCount: number;
  audioChapterCount: number;
  exporting?: "epub" | "markdown" | "txt" | "audio";
}>();
const emit = defineEmits<{
  close: [];
  export: [format: "epub" | "markdown" | "txt" | "audio"];
}>();
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <section class="modal glass export-modal" aria-labelledby="export-modal-title">
      <button
        class="close-button"
        title="关闭"
        :disabled="Boolean(exporting)"
        @click="emit('close')"
      >
        <X :size="20" />
      </button>
      <span class="modal-icon"><Download :size="22" /></span>
      <h2 id="export-modal-title">导出本地内容</h2>
      <p class="modal-description">选择一种格式下载当前书籍的本地内容。</p>
      <div class="export-options">
        <button
          class="export-option"
          :disabled="!textChapterCount || Boolean(exporting)"
          @click="emit('export', 'epub')"
        >
          <span class="option-icon"><FileDown :size="20" /></span>
          <span class="option-copy"><strong>EPUB</strong><small>适合阅读器导入</small></span>
          <span class="option-count">{{ textChapterCount }} 章</span>
        </button>
        <button
          class="export-option"
          :disabled="!textChapterCount || Boolean(exporting)"
          @click="emit('export', 'markdown')"
        >
          <span class="option-icon"><FileArchive :size="20" /></span>
          <span class="option-copy"><strong>Markdown ZIP</strong><small>包含正文和本地图片</small></span>
          <span class="option-count">{{ textChapterCount }} 章</span>
        </button>
        <button
          class="export-option"
          :disabled="!textChapterCount || Boolean(exporting)"
          @click="emit('export', 'txt')"
        >
          <span class="option-icon"><FileText :size="20" /></span>
          <span class="option-copy"><strong>TXT</strong><small>纯文本格式</small></span>
          <span class="option-count">{{ textChapterCount }} 章</span>
        </button>
        <button
          class="export-option"
          :disabled="!audioChapterCount || Boolean(exporting)"
          @click="emit('export', 'audio')"
        >
          <span class="option-icon audio-icon"><Headphones :size="20" /></span>
          <span class="option-copy"><strong>有声 ZIP</strong><small>包含音频和播放列表</small></span>
          <span class="option-count">{{ audioChapterCount }} 章</span>
        </button>
      </div>
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
  width: min(440px, 100%);
  padding: 28px;
  border-radius: 20px;
  background: rgba(255, 249, 245, 0.88);
}
.modal-icon,
.option-icon {
  display: grid;
  place-items: center;
}
.modal-icon {
  width: 42px;
  height: 42px;
  border-radius: 13px;
  color: var(--theme-color-dark);
  background: #ffe5d5;
}
.modal h2 {
  margin: 15px 0 5px;
  color: #69453d;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 23px;
}
.modal-description {
  margin: 0 0 18px;
  color: #99766c;
  font-size: 12px;
  line-height: 1.7;
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
.close-button:hover:not(:disabled) {
  color: var(--theme-color-dark);
  background: #fff0e7;
}
.close-button:disabled {
  cursor: wait;
  opacity: 0.7;
}
.export-options {
  display: grid;
  gap: 8px;
}
.export-option {
  display: grid;
  grid-template-columns: 38px minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-height: 64px;
  padding: 9px 11px;
  border: 1px solid #ead1c5;
  border-radius: 10px;
  color: #69453d;
  background: rgba(255, 250, 247, 0.78);
  text-align: left;
}
.export-option:hover:not(:disabled) {
  border-color: var(--theme-color);
  background: #fff1e9;
}
.export-option:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
.option-icon {
  width: 36px;
  height: 36px;
  border-radius: 9px;
  color: var(--theme-color-dark);
  background: #ffe8db;
}
.audio-icon {
  color: #a6535c;
  background: #fdf1f0;
}
.option-copy {
  display: grid;
  min-width: 0;
  gap: 3px;
}
.option-copy strong {
  overflow: hidden;
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.option-copy small,
.option-count {
  color: #a47b6d;
  font-size: 11px;
}
.option-count {
  white-space: nowrap;
}
@media (max-width: 500px) {
  .modal {
    padding: 22px 16px;
  }
  .export-option {
    grid-template-columns: 34px minmax(0, 1fr) auto;
    gap: 8px;
    padding-inline: 8px;
  }
}
</style>
