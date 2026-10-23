<script setup lang="ts">
import { inject } from "vue";
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
    @switch-mode="desk.switchChapterMode"
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
    @logout-app="desk.logoutApp"
    @logout-web="desk.logoutWeb"
  />
  <AccountProfileModal
    :open="desk.accountOpen.value"
    :auth="desk.auth.value"
    :profile="desk.accountProfile.value"
    :loading="desk.accountProfileLoading.value"
    :error="desk.accountProfileError.value"
    @close="desk.accountOpen.value = false"
    @logout="desk.logout"
    @manage-connections="desk.manageConnections"
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
</template>
