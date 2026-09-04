<script setup lang="ts">
import { inject } from "vue";
import { CheckCircle2 } from "lucide-vue-next";
import AccountProfileModal from "./AccountProfileModal.vue";
import AuthModal from "./AuthModal.vue";
import ChapterPickerModal from "./ChapterPickerModal.vue";
import ConfirmDeleteModal from "./ConfirmDeleteModal.vue";
import DownloadQueueModal from "./DownloadQueueModal.vue";
import SettingModal from "./SettingModal.vue";
import { deskInjectionKey } from "../deskContext";

const desk = inject(deskInjectionKey);
if (!desk) throw new Error("Desk context is unavailable");
</script>

<template>
  <ChapterPickerModal
    :open="desk.chapterModalOpen.value"
    :novel="desk.chapterNovel.value"
    :mode="desk.chapterMode.value"
    :loading="desk.chapterLoading.value"
    :volumes="desk.chapterVolumes.value"
    :audio-chapters="desk.audioChapters.value"
    :comic-chapters="desk.comicChapters.value"
    :selected-ids="desk.selectedChapterIds.value"
    :format-date="desk.formatDate"
    @close="desk.chapterModalOpen.value = false"
    @update:selected-ids="desk.selectedChapterIds.value = $event"
    @toggle-all="desk.toggleAllChapters"
    @confirm="desk.confirmChapterDownload"
  />
  <DownloadQueueModal
    :open="desk.queueOpen.value"
    :jobs="desk.jobs.value"
    @close="desk.queueOpen.value = false"
    @pause="desk.pauseJob"
    @resume="desk.resumeJob"
    @remove="desk.deleteJob"
  />
  <AuthModal
    :open="desk.credentialsOpen.value"
    :auth="desk.auth.value"
    :busy="desk.loginBusy.value"
    @close="desk.credentialsOpen.value = false"
    @login="desk.login"
    @browser-login="desk.browserLogin"
    @logout="desk.logout"
  />
  <AccountProfileModal
    :open="desk.accountOpen.value"
    :profile="desk.accountProfile.value"
    :loading="desk.accountProfileLoading.value"
    :error="desk.accountProfileError.value"
    @close="desk.accountOpen.value = false"
    @logout="desk.logout"
  />
  <SettingModal
    :open="desk.requestPolicyOpen.value"
    :policy="desk.requestPolicy.value"
    :saving="desk.requestPolicySaving.value"
    :dictionary-size="desk.contentDictionarySize.value"
    :dictionary-updating="desk.contentDictionaryUpdating.value"
    :dictionary-chapter-id="desk.contentDictionaryChapterId.value"
    @close="desk.requestPolicyOpen.value = false"
    @save="desk.saveRequestPolicy"
    @update-dictionary="desk.updateContentDictionary"
  />
  <ConfirmDeleteModal
    :book="desk.confirmBook.value"
    @close="desk.confirmBook.value = undefined"
    @confirm="desk.confirmDeleteBook"
  />
  <Transition name="toast">
    <div v-if="desk.toast.value" class="toast">
      <CheckCircle2 :size="18" />{{ desk.toast.value }}
    </div>
  </Transition>
</template>

<style scoped>
.toast {
  position: fixed;
  bottom: 22px;
  left: 50%;
  z-index: 30;
  display: flex;
  align-items: center;
  gap: 7px;
  max-width: calc(100vw - 32px);
  padding: 11px 14px;
  border-radius: 12px;
  color: #fff;
  background: #654139;
  box-shadow: 0 10px 25px #6d403550;
  font-size: 13px;
  transform: translateX(-50%);
}
.toast-enter-active,
.toast-leave-active { transition: 0.2s; }
.toast-enter-from,
.toast-leave-to { opacity: 0; transform: translate(-50%, 8px); }
@media (max-width: 760px) {
  .toast { bottom: calc(78px + env(safe-area-inset-bottom)); }
}
</style>
