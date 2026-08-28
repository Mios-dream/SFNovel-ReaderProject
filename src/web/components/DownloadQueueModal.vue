<script setup lang="ts">
import {
  CheckCircle2,
  ChevronRight,
  CloudDownload,
  LoaderCircle,
  Pause,
  Play,
  Trash2,
  X,
} from "lucide-vue-next";
import type { Job } from "../types";

defineProps<{ open: boolean; jobs: Job[] }>();
const emit = defineEmits<{
  close: [];
  pause: [job: Job];
  resume: [job: Job];
  remove: [job: Job];
}>();
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <section class="modal glass queue-modal">
      <button class="close-button" title="关闭" @click="emit('close')">
        <X :size="20" /></button
      ><span class="modal-icon"><CloudDownload :size="22" /></span>
      <h2>下载列表</h2>
      <p>进行中的任务可以暂停或继续，已完成或失败的任务可以删除。</p>
      <div v-if="!jobs.length" class="queue-empty">
        <CloudDownload :size="29" />
        <p>暂无下载任务</p>
      </div>
      <div v-else class="job-list">
        <article v-for="job in jobs" :key="job.id" class="job">
          <div class="job-row">
            <span :class="['job-status', job.status]"
              ><CheckCircle2 v-if="job.status === 'done'" :size="17" /><X
                v-else-if="job.status === 'error' || job.status === 'cancelled'"
                :size="17" /><LoaderCircle
                v-else
                class="spin"
                :size="17" /></span
            ><strong>{{ job.title }}</strong
            ><button
              v-if="job.status === 'downloading' || job.status === 'queued'"
              class="cancel-button"
              title="暂停下载"
              @click="emit('pause', job)"
            >
              <Pause :size="14" /></button
            ><button
              v-if="job.status === 'paused'"
              class="cancel-button resume-button"
              title="继续下载"
              @click="emit('resume', job)"
            >
              <Play :size="14" /></button
            ><button
              class="cancel-button delete-job-button"
              title="删除任务"
              @click="emit('remove', job)"
            >
              <Trash2 :size="14" />
            </button>
          </div>
          <p>{{ job.message }}</p>
          <div
            v-if="job.status !== 'error' && job.status !== 'cancelled'"
            class="progress"
          >
            <i :style="{ width: `${job.progress}%` }"></i>
          </div>
          <a v-if="job.file" :href="job.file" target="_blank" class="open-file"
            >打开文件 <ChevronRight :size="15"
          /></a>
        </article>
      </div>
    </section>
  </div>
</template>

<style scoped>
.modal-backdrop { position: fixed; inset: 0; z-index: 10; display: grid; padding: 18px; background: rgba(91, 49, 39, 0.24); place-items: center; }
.modal { position: relative; width: min(410px, 100%); padding: 28px; border-radius: 20px; background: rgba(255, 249, 245, 0.8); }
.queue-modal { width: min(680px, 100%); max-height: calc(100vh - 36px); overflow: auto; }
.modal h2 { margin: 15px 0 5px; color: #69453d; font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 23px; }
.modal > p { margin: 0 0 18px; color: #99766c; font-size: 12px; line-height: 1.7; }
.modal-icon { display: grid; width: 42px; height: 42px; border-radius: 13px; color: var(--theme-color-dark); background: #ffe5d5; place-items: center; }
.close-button { position: absolute; top: 16px; right: 16px; display: grid; width: 40px; height: 40px; border: 0; border-radius: 13px; color: #a17367; background: rgba(255, 255, 255, 0.7); place-items: center; transition: 0.2s; }
.close-button:hover { color: var(--theme-color-dark); background: #fff0e7; }
.queue-empty { margin: auto 0; color: #b28d82; font-size: 12px; text-align: center; }
.queue-empty svg { color: #e8ad8f; }
.job-list { display: grid; gap: 13px; }
.job { padding-bottom: 14px; border-bottom: 1px solid rgba(166, 100, 76, 0.15); }
.job-row { display: flex; align-items: center; gap: 8px; }
.job-row strong { flex: 1; overflow: hidden; color: #65433c; font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 14px; text-overflow: ellipsis; white-space: nowrap; }
.job-status { display: grid; flex: 0 0 auto; width: 23px; height: 23px; border-radius: 8px; color: var(--theme-color-dark); background: #ffe8d9; place-items: center; }
.job-status.error, .job-status.cancelled { color: #bb594f; background: #f7d6d0; }
.cancel-button { display: grid; flex: 0 0 auto; width: 25px; height: 25px; border: 0; border-radius: 8px; color: #b85f51; background: #f9ddd7; place-items: center; }
.cancel-button:hover { color: #fff; background: #e99585; }
.job p { margin: 7px 0; color: #a78378; font-size: 11px; }
.progress { height: 5px; overflow: hidden; border-radius: 99px; background: #f1ded5; }
.progress i { display: block; height: 100%; border-radius: inherit; background: var(--theme-color); transition: width 0.35s; }
.open-file { display: flex; align-items: center; gap: 2px; margin-top: 8px; color: var(--theme-color-dark); font-size: 11px; font-weight: 600; text-decoration: none; }

@media (max-width: 760px) { .queue-modal { max-height: calc(100vh - 20px); padding: 21px; } }
</style>
