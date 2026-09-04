<script setup lang="ts">
import {
  Download,
  FileArchive,
  FileDown,
  FileText,
  Headphones,
  Images,
  X,
} from "lucide-vue-next";

type Media = "novel" | "audio" | "comic";
type ExportFormat = "epub" | "markdown" | "txt" | "audio" | "comic";

const props = defineProps<{
  open: boolean;
  media: Media;
  textChapterCount: number;
  audioChapterCount: number;
  comicChapterCount: number;
  exporting?: ExportFormat;
}>();
const emit = defineEmits<{
  close: [];
  export: [format: ExportFormat];
}>();

const title = (media: Media) =>
  media === "novel" ? "小说" : media === "audio" ? "有声" : "漫画";
</script>

<template>
  <div v-if="props.open" class="modal-backdrop" @click.self="emit('close')">
    <section
      class="modal glass export-modal"
      aria-labelledby="export-modal-title"
    >
      <button
        class="close-button"
        title="关闭"
        :disabled="Boolean(props.exporting)"
        @click="emit('close')"
      >
        <X :size="20" />
      </button>
      <span class="modal-icon"><Download :size="22" /></span>
      <h2 id="export-modal-title">导出{{ title(props.media) }}</h2>
      <p class="modal-description">选择一种格式导出当前本地资源。</p>
      <div class="export-options">
        <template v-if="props.media === 'novel'">
          <button
            class="export-option"
            :disabled="!props.textChapterCount || Boolean(props.exporting)"
            @click="emit('export', 'epub')"
          >
            <span class="option-icon"><FileDown :size="20" /></span
            ><span class="option-copy"
              ><strong>EPUB</strong><small>适合阅读器导入</small></span
            ><span class="option-count">{{ props.textChapterCount }} 章</span>
          </button>
          <button
            class="export-option"
            :disabled="!props.textChapterCount || Boolean(props.exporting)"
            @click="emit('export', 'markdown')"
          >
            <span class="option-icon"><FileArchive :size="20" /></span
            ><span class="option-copy"
              ><strong>Markdown ZIP</strong
              ><small>包含正文和本地图片</small></span
            ><span class="option-count">{{ props.textChapterCount }} 章</span>
          </button>
          <button
            class="export-option"
            :disabled="!props.textChapterCount || Boolean(props.exporting)"
            @click="emit('export', 'txt')"
          >
            <span class="option-icon"><FileText :size="20" /></span
            ><span class="option-copy"
              ><strong>TXT</strong><small>纯文本格式</small></span
            ><span class="option-count">{{ props.textChapterCount }} 章</span>
          </button>
        </template>
        <button
          v-else-if="props.media === 'audio'"
          class="export-option"
          :disabled="!props.audioChapterCount || Boolean(props.exporting)"
          @click="emit('export', 'audio')"
        >
          <span class="option-icon audio-icon"><Headphones :size="20" /></span
          ><span class="option-copy"
            ><strong>有声 ZIP</strong><small>包含音频和播放列表</small></span
          ><span class="option-count">{{ props.audioChapterCount }} 章</span>
        </button>
        <button
          v-else
          class="export-option"
          :disabled="!props.comicChapterCount || Boolean(props.exporting)"
          @click="emit('export', 'comic')"
        >
          <span class="option-icon comic-icon"><Images :size="20" /></span
          ><span class="option-copy"
            ><strong>漫画 ZIP</strong><small>包含已下载漫画页面</small></span
          ><span class="option-count">{{ props.comicChapterCount }} 章</span>
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
  background: rgba(255, 249, 245, 0.94);
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
.comic-icon {
  color: #604a91;
  background: #f1ebfb;
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
}
</style>
