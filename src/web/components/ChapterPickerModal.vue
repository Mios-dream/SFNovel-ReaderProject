<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <section class="modal glass chapter-modal">
      <button class="close-button" title="关闭" @click="emit('close')">
        <X :size="20" />
      </button>
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
            ><Headphones v-if="mode === 'audio'" :size="22" /><BookOpen
              v-else
              :size="22"
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
        <LoaderCircle class="spin" :size="24" />正在读取书籍详情
      </div>
      <template v-else>
        <div
          v-if="hasAudio"
          class="chapter-tabs"
          role="tablist"
          aria-label="下载类型"
        >
          <button
            class="chapter-tab"
            :class="{ active: mode === 'text' }"
            role="tab"
            :aria-selected="mode === 'text'"
            @click="emit('change-mode', 'text')"
          >
            <BookOpen :size="16" />下载章节</button
          ><button
            class="chapter-tab"
            :class="{ active: mode === 'audio' }"
            role="tab"
            :aria-selected="mode === 'audio'"
            @click="emit('change-mode', 'audio')"
          >
            <Headphones :size="16" />下载有声小说
          </button>
        </div>
        <button
          class="select-all"
          :disabled="!selectableIds.length"
          @click="emit('toggleAll')"
        >
          {{ allSelected ? "取消全选" : "全选可下载内容" }}（{{
            selectedCount
          }}/{{ selectableIds.length }}）
        </button>
        <div class="chapter-list">
          <template v-if="mode === 'text'"
            ><div
              v-for="volume in volumes"
              :key="volume.volumeId"
              class="chapter-volume"
            >
              <div class="volume-heading">
                <strong>{{ volume.title }}</strong>
                <button
                  class="volume-select-all"
                  :disabled="!selectableVolumeIds(volume).length"
                  @click="toggleVolume(volume)"
                >
                  {{ isVolumeSelected(volume) ? "取消全选" : "全选本卷" }}（{{
                    selectedVolumeCount(volume)
                  }}/{{ selectableVolumeIds(volume).length }}）
                </button>
              </div>
              <label
                v-for="chapter in volume.chapters"
                :key="chapter.chapId"
                :class="{
                  downloaded: chapter.downloaded,
                  locked: chapter.isVip && !chapter.isUnlocked,
                }"
                ><input
                  :checked="selectedIds.includes(chapter.chapId)"
                  :disabled="!canDownload(chapter)"
                  type="checkbox"
                  :value="chapter.chapId"
                  @change="
                    toggleChapter(
                      chapter.chapId,
                      ($event.target as HTMLInputElement).checked,
                    )
                  "
                /><span class="chapter-title">{{ chapter.title }}</span
                ><span
                  v-if="chapter.downloaded"
                  class="chapter-state downloaded"
                  ><CheckCircle2 :size="15" />已下载</span
                ><span
                  v-else-if="chapter.isVip"
                  class="chapter-state vip"
                  :class="{ unlocked: chapter.isUnlocked }"
                  ><LockKeyholeOpen
                    v-if="chapter.isUnlocked"
                    :size="15"
                  /><LockKeyhole v-else :size="15" />{{
                    chapter.isUnlocked
                      ? "已解锁"
                      : `VIP ${chapter.needFireMoney} 火券`
                  }}</span
                ></label
              >
            </div></template
          ><template v-else
            ><label
              v-for="chapter in audioChapters"
              :key="chapter.id"
              :class="{ downloaded: chapter.downloaded }"
              ><input
                :checked="selectedIds.includes(chapter.id)"
                :disabled="chapter.downloaded"
                type="checkbox"
                :value="chapter.id"
                @change="
                  toggleChapter(
                    chapter.id,
                    ($event.target as HTMLInputElement).checked,
                  )
                "
              /><span class="chapter-title"
                >{{ chapter.volume }} - {{ chapter.title }}</span
              ><span v-if="chapter.downloaded" class="chapter-state downloaded"
                ><CheckCircle2 :size="15" />已下载</span
              ></label
            ></template
          >
        </div>
        <button
          class="primary-button full"
          :disabled="!selectedCount"
          @click="emit('confirm')"
        >
          {{ mode === "audio" ? "下载选中有声内容" : "下载选中章节" }}
        </button></template
      >
    </section>
  </div>
</template>
<script setup lang="ts">
import { computed } from "vue";
import {
  BookOpen,
  CheckCircle2,
  Headphones,
  LoaderCircle,
  LockKeyhole,
  LockKeyholeOpen,
  X,
} from "lucide-vue-next";
import type { Chapter, ChapterMode, ChapterVolume, Novel } from "../types";

const props = defineProps<{
  open: boolean;
  novel?: Novel;
  mode: ChapterMode;
  loading: boolean;
  hasAudio: boolean;
  volumes: ChapterVolume[];
  audioChapters: Chapter[];
  selectedIds: number[];
  formatDate: (value: string) => string;
}>();
const emit = defineEmits<{
  close: [];
  "update:selectedIds": [value: number[]];
  toggleAll: [];
  "change-mode": [value: ChapterMode];
  confirm: [];
}>();

type TextChapter = ChapterVolume["chapters"][number];
type SelectableChapter = (TextChapter | Chapter) & {
  downloaded?: boolean;
  isVip?: boolean;
  isUnlocked?: boolean;
};

// 文本和有声章节统一转换为可选择结构，模板无需区分两种目录来源。
/**
 * 判断章节是否允许加入下载任务。
 * @param chapter 待判断的文本或有声章节。
 * @returns 未下载且已解锁（或免费）时返回 true。
 */
function canDownload(chapter: SelectableChapter) {
  return !chapter.downloaded && (!chapter.isVip || chapter.isUnlocked);
}

const allChapters = computed<SelectableChapter[]>(() =>
  props.mode === "text"
    ? props.volumes.flatMap((volume) => volume.chapters)
    : props.audioChapters,
);
const selectableIds = computed(() =>
  allChapters.value
    .filter(canDownload)
    .map((chapter) => ("chapId" in chapter ? chapter.chapId : chapter.id)),
);
const selectedCount = computed(
  () =>
    props.selectedIds.filter((id) => selectableIds.value.includes(id)).length,
);
const allSelected = computed(
  () =>
    selectableIds.value.length > 0 &&
    selectedCount.value === selectableIds.value.length,
);

/**
 * 获取卷内所有可下载章节 ID。
 * @param volume 要检查的章节卷。
 * @returns 该卷中可下载章节的 ID 数组。
 */
function selectableVolumeIds(volume: ChapterVolume) {
  return volume.chapters.filter(canDownload).map((chapter) => chapter.chapId);
}

/**
 * 统计卷内已选章节数量。
 * @param volume 要统计的章节卷。
 * @returns 当前已选章节数。
 */
function selectedVolumeCount(volume: ChapterVolume) {
  const ids = selectableVolumeIds(volume);
  return props.selectedIds.filter((id) => ids.includes(id)).length;
}

/**
 * 判断章节卷是否已全选。
 * @param volume 要判断的章节卷。
 * @returns 卷内存在可选章节且全部已选时返回 true。
 */
function isVolumeSelected(volume: ChapterVolume) {
  const ids = selectableVolumeIds(volume);
  return ids.length > 0 && selectedVolumeCount(volume) === ids.length;
}

/**
 * 切换整卷章节的选择状态。
 * @param volume 要切换的章节卷。
 * @returns 无返回值；通过事件通知父组件更新选中 ID。
 */
function toggleVolume(volume: ChapterVolume) {
  const ids = selectableVolumeIds(volume);
  const nextIds = isVolumeSelected(volume)
    ? props.selectedIds.filter((id) => !ids.includes(id))
    : [...new Set([...props.selectedIds, ...ids])];
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
.chapter-modal {
  width: min(760px, 100%);
  max-height: calc(100vh - 36px);
  overflow: auto;
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
  /* margin: 16px 0 0 !important; */
  /* padding: 10px 12px; */
  overflow: auto;
  font-size: 14px;
  /* border: 1px solid rgba(255, 255, 255, 0.72); */
  /* border-radius: 9px; */
  color: white;
  /* background: rgba(255, 249, 245, 0.9); */
  /* box-shadow: 0 5px 13px rgba(73, 39, 30, 0.12); */
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
.chapter-tabs {
  display: flex;
  gap: 7px;
  margin: 12px 0 6px;
  padding: 5px;
  border-radius: 10px;
  background: #f8e5da;
}
.chapter-tab {
  display: inline-flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-width: 0;
  min-height: 35px;
  border: 0;
  border-radius: 7px;
  color: #986d60;
  background: transparent;
  font-size: 12px;
  font-weight: 600;
}
.chapter-tab.active {
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 3px 8px rgba(165, 79, 49, 0.2);
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
  margin: 8px 0;
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
  display: flex;
  flex: 1;
  flex-direction: column;
  /* height: auto; */
  height: 50vh;
  max-height: 300px;
  min-height: 100px;
  padding-right: 5px;
  overflow: auto;
}

.chapter-volume {
  padding: 13px 0;
  border-bottom: 1px solid #efd8ce;
}
.volume-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.volume-heading > strong {
  color: #80594f;
  font-size: 12px;
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
.chapter-list label {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 9px 0;
  font-size: 13px;
}
.chapter-list label.downloaded,
.chapter-list label.locked {
  color: #b5968b;
}
.chapter-list label:has(input:disabled) {
  cursor: not-allowed;
}
.chapter-title {
  flex: 1;
  min-width: 0;
}
.chapter-state {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 3px;
  font-size: 11px;
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
  .chapter-modal {
    max-height: calc(100vh - 20px);
    padding: 21px;
  }
  .chapter-hero {
    margin: -21px -21px 15px;
    padding: 20px 55px 18px 21px;
  }
  .chapter-list {
    max-height: calc(100vh - 260px);
  }
}
</style>
