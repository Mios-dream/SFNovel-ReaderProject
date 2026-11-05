<script setup lang="ts">
import AccountProfileModal from "./AccountProfileModal.vue";
import AuthModal from "./AuthModal.vue";
import ChapterPickerModal from "./ChapterPickerModal.vue";
import ConfirmDeleteModal from "./ConfirmDeleteModal.vue";
import DownloadQueueModal from "./DownloadQueueModal.vue";
import SettingModal from "./SettingModal.vue";
import SigninModal from "./SigninModal.vue";
import UpdateModal from "./UpdateModal.vue";

import { useAuth, useJobs, useLibrary, usePicker, useSettings, useUpdater } from "../desk";
import { formatMonthDay } from "../utils/format";

const auth = useAuth();
const queue = useJobs();
const library = useLibrary();
const picker = usePicker();
const settings = useSettings();
const updater = useUpdater();

</script>

<template>
  <ChapterPickerModal
    :open="picker.chapterModalOpen.value"
    :novel="picker.chapterNovel.value"
    :mode="picker.chapterMode.value"
    :loading="picker.chapterLoading.value"
    :volumes="picker.chapterVolumes.value"
    :audio-chapters="picker.audioChapters.value"
    :comic-chapters="picker.comicChapters.value"
    :selected-ids="picker.selectedChapterIds.value"
    :format-date="formatMonthDay"
    @close="picker.chapterModalOpen.value = false"
    @update:selected-ids="picker.selectedChapterIds.value = $event"
    @toggle-all="picker.toggleAllChapters"
    @confirm="picker.confirmChapterDownload"
    @switch-mode="picker.switchChapterMode"
  />
  <DownloadQueueModal
    :open="queue.queueOpen.value"
    :jobs="queue.jobs.value"
    @close="queue.queueOpen.value = false"
    @pause="queue.pauseJob"
    @resume="queue.resumeJob"
    @remove="queue.deleteJob"
  />
  <AuthModal
    :open="auth.credentialsOpen.value"
    :auth="auth.auth.value"
    :initial-tab="auth.credentialsTab.value"
    :busy="auth.loginBusy.value"
    @close="auth.credentialsOpen.value = false"
    @login="auth.login"
    @browser-login="auth.browserLogin"
    @logout="auth.logout"
    @logout-app="auth.logoutApp"
    @logout-web="auth.logoutWeb"
  />
  <AccountProfileModal
    :open="auth.accountOpen.value"
    :auth="auth.auth.value"
    :profile="auth.accountProfile.value"
    :loading="auth.accountProfileLoading.value"
    :error="auth.accountProfileError.value"
    @close="auth.accountOpen.value = false"
    @logout="auth.logout"
    @signin="auth.openSignin"
    @manage-connections="auth.manageConnections"
  />
  <SigninModal
    :open="auth.signinOpen.value"
    @close="auth.signinOpen.value = false"
  />
  <SettingModal
    :open="settings.settingsOpen.value"
    :settings="settings.settings.value"
    :saving="settings.settingsSaving.value"
    :dictionary-size="settings.contentDictionarySize.value"
    :dictionary-updating="settings.contentDictionaryUpdating.value"
    :dictionary-chapter-id="settings.contentDictionaryChapterId.value"
    :current-version="updater.currentVersion.value"
    :update-status="updater.status.value"
    :update-version="updater.info.value?.version"
    @close="settings.settingsOpen.value = false"
    @save="settings.saveSettings"
    @update-dictionary="settings.updateContentDictionary"
    @check-update="updater.checkFromSettings"
    @show-update="updater.showUpdate"
    @open-releases="updater.openReleases"
  />
  <UpdateModal
    :open="updater.modalOpen.value"
    :status="updater.status.value"
    :info="updater.info.value"
    :current-version="updater.currentVersion.value"
    :error="updater.error.value"
    @close="updater.closeModal"
    @retry="updater.checkForUpdate(true)"
    @open-releases="updater.openReleases"
  />
  <ConfirmDeleteModal
    :book="library.confirmBook.value"
    @close="library.confirmBook.value = undefined"
    @confirm="library.confirmDeleteBook"
  />
</template>
