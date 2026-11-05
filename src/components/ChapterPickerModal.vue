<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <section class="modal glass chapter-modal">
      <header
        class="chapter-hero"
        :class="{ 'has-cover': Boolean(novel?.novelCover) }"
        :style="
          novel?.novelCover
            ? { backgroundImage: `url(${novel.novelCover})` }
            : undefined
        "
      >
        <div class="chapter-hero-content">
          <span class="modal-icon"
            ><Headphones v-if="mode === 'audio'" :size="22" /><Image
              v-else-if="mode === 'comic'"
              :size="22" /><BookOpen v-else :size="22"
          /></span>
          <div>
            <h2>{{ novel?.novelName }}</h2>
            <p class="chapter-subtitle">
              {{ novel?.authorName }} ·
              {{ novel?.isFinish ? "已完结" : "连载中" }} · 更新于
              {{ novel ? formatDate(novel.lastUpdateTime) : "" }}
            </p>
          </div>
        </div>
        <p v-if="novel?.description" class="novel-description">
          {{ novel.description }}
        </p>
      </header>
      <div v-if="loading" class="chapter-loading">
        <LoaderCircle class="spin" :size="24" />正在读取章节目录
      </div>
      <template v-else>
        <nav
          v-if="downloadModes.length > 1"
          class="download-tabs"
          aria-label="下载类型"
        >
          <button
            v-for="item in downloadModes"
            :key="item.mode"
            class="download-tab"
            :class="{ active: mode === item.mode }"
            @click="emit('switch-mode', item.mode)"
          >
            <BookOpen v-if="item.mode === 'novel'" :size="15" />
            <Headphones v-else :size="15" />{{ item.label }}
          </button>
        </nav>
        <button
          class="select-all"
          :disabled="!selectableIds.length"
          @click="emit('toggleAll')"
        >
          {{ allSelected ? "取消全选" : "全选可下载内容" }}（{{
            selectedCount
          }}/{{ selectableIds.length }}）
        </button>
        <div
          ref="chapterList"
          class="chapter-list"
          @scroll.passive="handleChapterScroll"
        >
          <div
            :style="{ height: `${chapterWindow.top}px` }"
            aria-hidden="true"
          ></div>
          <template v-for="row in chapterWindow.rows" :key="row.key">
            <div v-if="row.kind === 'volume'" class="picker-volume-row">
              <strong>{{ row.title }}</strong>
              <button
                class="volume-select-all"
                :disabled="!row.ids.length"
                @click="togglePickerVolume(row)"
              >
                {{ row.allSelected ? "取消全选" : "全选本卷" }}（{{
                  row.selectedCount
                }}/{{ row.ids.length }}）
              </button>
            </div>
            <label
              v-else
              class="picker-chapter-row"
              :class="{ downloaded: row.downloaded }"
              ><input
                :checked="selectedIdSet.has(row.id)"
                :disabled="!row.canSelect"
                type="checkbox"
                :value="row.id"
                @change="
                  toggleChapter(
                    row.id,
                    ($event.target as HTMLInputElement).checked,
                  )
                "
              /><span class="chapter-title">{{ row.title }}</span
              ><span v-if="row.downloaded" class="chapter-state downloaded"
                ><CheckCircle2 :size="15" />已下载</span
              ><span
                v-else-if="row.isVip"
                class="chapter-state vip"
                :class="{ unlocked: row.unlocked }"
                ><LockKeyholeOpen
                  v-if="row.unlocked"
                  :size="15"
                  aria-label="已解锁"
                /><LockKeyhole
                  v-else
                  :size="15"
                  aria-label="VIP 章节"
                />{{ row.lockLabel }}</span
              ></label
            >
          </template>
          <div
            :style="{ height: `${chapterWindow.bottom}px` }"
            aria-hidden="true"
          ></div>
        </div>
        <button
          class="primary-button full"
          :disabled="!selectedCount"
          @click="emit('confirm')"
        >
          {{
            mode === "audio"
              ? "下载选中有声内容"
              : mode === "comic"
                ? "下载选中漫画"
                : "下载选中章节"
          }}
        </button></template
      >
    </section>
  </div>
</template>
<script setup lang="ts">
import { computed, nextTick, watch } from "vue";
import {
  BookOpen,
  CheckCircle2,
  Headphones,
  Image,
  LoaderCircle,
  LockKeyhole,
  LockKeyholeOpen,
} from "@lucide/vue";
import { useVirtualRows } from "../composables/useVirtualRows";
import type { Chapter, ChapterVolume, MediaType, Novel } from "../types";

const props = defineProps<{
  open: boolean;
  novel?: Novel;
  mode: MediaType;
  loading: boolean;
  volumes: ChapterVolume[];
  audioChapters: Chapter[];
  comicChapters: Chapter[];
  selectedIds: number[];
  formatDate: (value: string) => string;
}>();
const emit = defineEmits<{
  close: [];
  "update:selectedIds": [value: number[]];
  toggleAll: [];
  confirm: [];
  "switch-mode": [mode: MediaType];
}>();

type TextChapter = ChapterVolume["chapters"][number];
type SelectableChapter = (TextChapter | Chapter) & {
  downloaded?: boolean;
  isVip?: boolean;
  isUnlocked?: boolean | "unknown";
  accessState?: TextChapter["accessState"];
};
type AudioVolume = { title: string; chapters: Chapter[] };
type PickerVolumeRow = {
  kind: "volume";
  key: string;
  height: number;
  title: string;
  ids: number[];
  selectedCount: number;
  allSelected: boolean;
};
type PickerChapterRow = {
  kind: "chapter";
  key: string;
  height: number;
  id: number;
  title: string;
  downloaded: boolean;
  isVip: boolean;
  unlocked: boolean;
  lockLabel: string;
  canSelect: boolean;
};
type PickerRow = PickerVolumeRow | PickerChapterRow;

const PICKER_VOLUME_HEIGHT = 44;
const PICKER_CHAPTER_HEIGHT = 38;

// 文本、有声和漫画章节统一转换为可选择结构，模板无需区分目录来源。
/**
 * 判断章节是否允许加入下载任务。
 * @param chapter 待判断的文本、有声或漫画章节。
 * @returns 未下载，且目录未明确标记为锁定时返回 true。
 */
function canDownload(chapter: SelectableChapter) {
  if (chapter.downloaded) return false;
  if (props.mode === "novel") return chapter.accessState === "available";
  if (props.mode === "comic") return chapter.isUnlocked !== false;
  return !chapter.isVip || chapter.isUnlocked === true;
}

const allChapters = computed<SelectableChapter[]>(() =>
  props.mode === "novel"
    ? props.volumes.flatMap((volume) => volume.chapters)
    : props.mode === "audio"
      ? props.audioChapters
      : props.comicChapters,
);
const audioVolumes = computed<AudioVolume[]>(() => {
  const grouped = new Map<string, AudioVolume>();
  for (const chapter of props.audioChapters) {
    const title = chapter.volume.trim() || "未分卷";
    const volume = grouped.get(title);
    if (volume) volume.chapters.push(chapter);
    else grouped.set(title, { title, chapters: [chapter] });
  }
  return [...grouped.values()];
});
const downloadModes = computed(() => {
  const modes: Array<{ mode: MediaType; label: string }> = [];
  if (props.volumes.length) modes.push({ mode: "novel", label: "小说" });
  if (props.audioChapters.length) modes.push({ mode: "audio", label: "有声" });
  return modes;
});
const selectedIdSet = computed(() => new Set(props.selectedIds));
const selectableIds = computed(() =>
  allChapters.value
    .filter(canDownload)
    .map((chapter) => ("chapId" in chapter ? chapter.chapId : chapter.id)),
);
const selectableIdSet = computed(() => new Set(selectableIds.value));
const selectedCount = computed(
  () =>
    props.selectedIds.reduce(
      (count, id) => count + (selectableIdSet.value.has(id) ? 1 : 0),
      0,
    ),
);
const allSelected = computed(
  () =>
    selectableIds.value.length > 0 &&
    selectedCount.value === selectableIds.value.length,
);

function pickerVolumeRow(
  key: string,
  title: string,
  ids: number[],
  selected: Set<number>,
): PickerVolumeRow {
  const selectedCount = ids.reduce(
    (count, id) => count + (selected.has(id) ? 1 : 0),
    0,
  );
  return {
    kind: "volume",
    key,
    height: PICKER_VOLUME_HEIGHT,
    title,
    ids,
    selectedCount,
    allSelected: ids.length > 0 && selectedCount === ids.length,
  };
}

function pickerChapterRow(
  key: string,
  id: number,
  title: string,
  chapter: SelectableChapter,
): PickerChapterRow {
  const unlocked =
    props.mode === "novel"
      ? chapter.accessState === "available"
      : chapter.isUnlocked === true;
  return {
    kind: "chapter",
    key,
    height: PICKER_CHAPTER_HEIGHT,
    id,
    title,
    downloaded: chapter.downloaded === true,
    isVip: chapter.isVip === true,
    unlocked,
    lockLabel:
      props.mode === "comic"
        ? unlocked
          ? "已解锁"
          : "VIP 章节"
        : props.mode === "novel" && unlocked
          ? "已解锁"
          : "VIP",
    canSelect: canDownload(chapter),
  };
}

const pickerRows = computed<PickerRow[]>(() => {
  const selected = selectedIdSet.value;
  const rows: PickerRow[] = [];
  if (props.mode === "novel") {
    props.volumes.forEach((volume) => {
      const ids = volume.chapters
        .filter(canDownload)
        .map((chapter) => chapter.chapId);
      rows.push(
        pickerVolumeRow(
          `text-volume-${volume.volumeId}`,
          volume.title,
          ids,
          selected,
        ),
      );
      for (const chapter of volume.chapters)
        rows.push(
          pickerChapterRow(
            `text-chapter-${chapter.chapId}`,
            chapter.chapId,
            chapter.title,
            chapter,
          ),
        );
    });
  } else if (props.mode === "audio") {
    audioVolumes.value.forEach((volume, index) => {
      const ids = volume.chapters
        .filter(canDownload)
        .map((chapter) => chapter.id);
      rows.push(
        pickerVolumeRow(`audio-volume-${index}`, volume.title, ids, selected),
      );
      for (const chapter of volume.chapters)
        rows.push(
          pickerChapterRow(
            `audio-chapter-${chapter.id}`,
            chapter.id,
            chapter.title,
            chapter,
          ),
        );
    });
  } else {
    for (const chapter of props.comicChapters)
      rows.push(
        pickerChapterRow(
          `comic-chapter-${chapter.id}`,
          chapter.id,
          chapter.title,
          chapter,
        ),
      );
  }
  return rows;
});

const {
  container: chapterList,
  windowed: chapterWindow,
  handleScroll: handleChapterScroll,
  reset: resetChapterList,
} = useVirtualRows(() => pickerRows.value);

watch(
  () => [props.open, props.mode, props.loading],
  async () => {
    if (!props.open || props.loading) return;
    await nextTick();
    if (chapterList.value) resetChapterList();
  },
  { flush: "post" },
);

function togglePickerVolume(row: PickerVolumeRow) {
  const ids = new Set(row.ids);
  const nextIds = row.allSelected
    ? props.selectedIds.filter((id) => !ids.has(id))
    : [...new Set([...props.selectedIds, ...row.ids])];
  emit("update:selectedIds", nextIds);
}

/**
 * 更新单个章节的勾选状态。
 * @param id 章节 ID。
 * @param checked 是否选中。
 * @returns 无返回值。
 */
function toggleChapter(id: number, checked: boolean) {
  emit(
    "update:selectedIds",
    checked
      ? [...props.selectedIds, id]
      : props.selectedIds.filter((selectedId) => selectedId !== id),
  );
}
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1000;
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
  display: flex;
  flex-direction: column;
}
.chapter-modal {
  width: min(760px, 100%);
  max-height: 80%;
  overflow: auto;
  scrollbar-width: auto;
  scrollbar-color: #edb18d transparent;
}

.chapter-hero {
  position: relative;
  min-height: 170px;
  margin: -28px -28px 16px;
  padding: 24px 21px 21px 28px;
  overflow: hidden;
  border-radius: 20px 20px 14px 14px;
  background: #ffe5d5;
  background-position: center 36%;
  background-size: cover;
}
.chapter-hero.has-cover::before {
  position: absolute;
  inset: 0;
  background: linear-gradient(
    0deg,
    rgba(75, 42, 34, 0.7),
    rgba(89, 51, 41, 0.5)
  );
  content: "";
}
.chapter-hero-content {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: 13px;
  min-width: 0;
}
.chapter-hero .novel-description {
  position: relative;
  z-index: 1;
  max-height: 72px;
  overflow: auto;
  scrollbar-width: auto;
  scrollbar-color: #edb18d transparent;
  font-size: 14px;
  color: white;
}
.modal h2 {
  margin: 0 0 5px;
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
  flex: 0 0 auto;
  display: grid;
  width: 42px;
  height: 42px;
  border-radius: 13px;
  color: var(--theme-color-dark);
  background: #ffe5d5;
  place-items: center;
}
.download-tabs {
  display: flex;
  gap: 6px;
  /* padding: 4px; */
  border-radius: 10px;
  background: #f8e8df;
  border: 1px solid #f0d9d0;
}
.download-tab {
  display: inline-flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-height: 34px;
  border: 0;
  border-radius: 7px;
  color: #94675c;
  background: transparent;
  font-size: 12px;
  font-weight: 700;
}
.download-tab.active {
  color: #fff;
  background: var(--theme-color);
}
.chapter-subtitle {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chapter-hero.has-cover h2,
.chapter-hero.has-cover .chapter-subtitle {
  color: #fff;
  text-shadow: 0 1px 4px rgba(51, 28, 22, 0.5);
}
.chapter-hero.has-cover .modal-icon {
  color: #fff;
  background: rgba(255, 246, 241, 0.22);
  backdrop-filter: blur(5px);
}
.chapter-loading {
  display: grid;
  min-height: 320px;
  gap: 10px;
  color: #a78378;
  font-size: 12px;
  justify-items: center;
  place-content: center;
}
.select-all {
  width: 100%;
  margin-top: 8px;
  padding: 8px 11px;
  border: 0;
  border-radius: 9px;
  color: var(--theme-color-dark);
  background: #ffe7d8;
  font-size: 12px;
  text-align: left;
}
.select-all:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.chapter-list {
  flex: 1;
  height: auto;
  max-height: 260px;
  min-height: 100px;
  padding-right: 5px;
  overflow: auto;
  scrollbar-width: auto;
  scrollbar-color: #edb18d transparent;
}

.picker-volume-row {
  display: flex;
  height: 44px;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  border-bottom: 1px solid #efd8ce;
}
.picker-volume-row > strong {
  min-width: 0;
  overflow: hidden;
  color: #80594f;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.volume-select-all {
  flex: 0 0 auto;
  min-height: 27px;
  padding: 0 8px;
  border: 0;
  border-radius: 7px;
  color: #9b6252;
  background: #fff0e7;
  font-size: 11px;
}
.volume-select-all:hover:not(:disabled) {
  color: #fff;
  background: var(--theme-color);
}
.volume-select-all:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.picker-chapter-row {
  display: flex;
  height: 38px;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: 13px;
}
.picker-chapter-row.downloaded {
  color: #b5968b;
}
.picker-chapter-row:has(input:disabled) {
  cursor: not-allowed;
}
.chapter-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chapter-state {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 3px;
  font-size: 12px;
  white-space: nowrap;
}
.chapter-state.downloaded {
  color: #5d9275;
}
.chapter-state.vip {
  color: #a56d4a;
}
.chapter-state.vip.unlocked {
  color: #5a8d77;
}
.chapter-list input {
  accent-color: var(--theme-color);
}
.primary-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 39px;
  padding: 0 16px;
  border: 0;
  border-radius: 10px;
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 7px 12px #e2946440;
  font-size: 13px;
  font-weight: 600;
  transition: 0.2s;
}
.primary-button:hover {
  background: var(--theme-color-dark);
  transform: translateY(-1px);
}
.primary-button:disabled {
  cursor: wait;
  opacity: 0.7;
}
.full {
  width: 100%;
  margin-top: 8px;
}

@media (max-width: 760px) {
  .chapter-hero {
    display: none;
  }
}
</style>
