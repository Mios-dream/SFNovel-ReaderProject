<template>
  <main
    class="app-shell"
    :class="{
      'immersive-active':
        desk.active === 'libraryDetail' ||
        desk.active === 'reader' ||
        desk.active === 'audioPlayer',
    }"
  >
    <AppSidebar
      :class="{
        'immersive-sidebar':
          desk.active === 'libraryDetail' ||
          desk.active === 'reader' ||
          desk.active === 'audioPlayer',
      }"
      :active="desk.active"
      :bookshelf-count="desk.bookshelf.length"
      :library-count="desk.library.length"
      :auth="desk.auth"
      :profile="desk.accountProfile"
      @navigate="desk.navigate"
      @account="desk.openAccount"
      @request-settings="desk.openRequestPolicy"
    />

    <section class="content">
      <header
        v-if="
          desk.active !== 'libraryDetail' &&
          desk.active !== 'reader' &&
          desk.active !== 'audioPlayer'
        "
        class="topbar"
      >
        <div>
          <p class="eyebrow">PERSONAL NOVEL DESK</p>
          <h1>
            {{ titleContent }}
          </h1>
        </div>
        <div class="topbar-actions">
          <button
            class="icon-button"
            title="请求设置"
            @click="desk.openRequestPolicy"
          >
            <Settings2 :size="20" /></button
          ><button
            class="icon-button open-library-button"
            title="打开输出目录"
            @click="desk.navigate('library')"
          >
            <FolderOpen :size="20" />
          </button>
        </div>
      </header>

      <DiscoverPage
        v-if="desk.active === 'discover'"
        v-model:query="desk.query"
        :results="desk.results"
        :loading="desk.loading"
        :searched="desk.searched"
        :format-date="desk.formatDate"
        @search="desk.search"
        @select="desk.openChapterPicker"
      />
      <BookshelfPage
        v-else-if="desk.active === 'bookshelf'"
        :novels="desk.bookshelf"
        :visible-novels="desk.pagedBookshelf"
        :loading="desk.bookshelfLoading"
        :categories="desk.bookshelfCategories"
        :active-category="desk.bookshelfCategory"
        :page="desk.bookshelfPage"
        :total-pages="desk.bookshelfTotalPages"
        :filtered-count="desk.filteredBookshelf.length"
        :format-date="desk.formatDate"
        @refresh="desk.refreshBookshelf"
        @category="desk.selectBookshelfCategory"
        @page="desk.setBookshelfPage"
        @select="desk.openChapterPicker"
      />
      <LibraryPage
        v-else-if="desk.active === 'library'"
        v-model:managing="desk.libraryManaging"
        :books="desk.library"
        :jobs="desk.libraryJobs"
        :format-date="desk.formatDate"
        @open="desk.openLibraryBook"
        @remove="desk.deleteBook"
        @pause="desk.pauseJob"
        @resume="desk.resumeJob"
        @discover="desk.navigate('discover')"
      />
      <LocalBookDetailPage
        v-else-if="desk.active === 'libraryDetail' && desk.localBook"
        :book="desk.localBook"
        :exporting="desk.exportingFormat"
        @back="desk.backFromLibraryDetail"
        @read="desk.openLocalChapter"
        @play-audio="desk.openLocalAudioPlayer"
        @online="desk.readOnline"
        @continue-download="desk.continueDownload"
        @export="desk.exportBook"
      />
      <LocalReaderPage
        v-else-if="
          desk.active === 'reader' && desk.localBook && desk.localChapter
        "
        :book-name="desk.localBook.name"
        :image-directory="desk.localBook.imageDirectory"
        :chapter="desk.localChapter"
        @back="desk.navigate('libraryDetail')"
      />
      <LocalAudioPlayerPage
        v-else-if="desk.active === 'audioPlayer' && desk.localBook"
        :book-name="desk.localBook.name"
        :author="desk.localBook.author"
        :cover="desk.localBook.cover"
        :tracks="desk.localBook.audioTracks"
        :initial-track-index="desk.localAudioTrackIndex"
        @back="desk.navigate('libraryDetail')"
      />
    </section>

    <button
      class="queue-fab"
      title="打开下载列表"
      @click="desk.queueOpen = true"
    >
      <CloudDownload :size="22" /><span
        v-if="desk.runningJobs.length"
        class="queue-count"
        >{{ desk.runningJobs.length }}</span
      >
    </button>

    <ChapterPickerModal
      :open="desk.chapterModalOpen"
      :novel="desk.chapterNovel"
      :mode="desk.chapterMode"
      :loading="desk.chapterLoading"
      :has-audio="desk.chapterHasAudio"
      :volumes="desk.chapterVolumes"
      :audio-chapters="desk.audioChapters"
      :selected-ids="desk.selectedChapterIds"
      :format-date="desk.formatDate"
      @close="desk.chapterModalOpen = false"
      @update:selected-ids="desk.selectedChapterIds = $event"
      @change-mode="desk.changeChapterMode"
      @toggle-all="desk.toggleAllChapters"
      @confirm="desk.confirmChapterDownload"
    />
    <DownloadQueueModal
      :open="desk.queueOpen"
      :jobs="desk.jobs"
      @close="desk.queueOpen = false"
      @pause="desk.pauseJob"
      @resume="desk.resumeJob"
      @remove="desk.deleteJob"
    />
    <AuthModal
      :open="desk.credentialsOpen"
      :auth="desk.auth"
      :busy="desk.loginBusy"
      @close="desk.credentialsOpen = false"
      @login="desk.login"
      @browser-login="desk.browserLogin"
      @logout="desk.logout"
    />
    <AccountProfileModal
      :open="desk.accountOpen"
      :profile="desk.accountProfile"
      :loading="desk.accountProfileLoading"
      :error="desk.accountProfileError"
      @close="desk.accountOpen = false"
      @logout="desk.logout"
    />
    <SettingModal
      :open="desk.requestPolicyOpen"
      :policy="desk.requestPolicy"
      :saving="desk.requestPolicySaving"
      :dictionary-size="desk.contentDictionarySize"
      :dictionary-updating="desk.contentDictionaryUpdating"
      :dictionary-chapter-id="desk.contentDictionaryChapterId"
      @close="desk.requestPolicyOpen = false"
      @save="desk.saveRequestPolicy"
      @update-dictionary="desk.updateContentDictionary"
    />
    <ConfirmDeleteModal
      :book="desk.confirmBook"
      @close="desk.confirmBook = undefined"
      @confirm="desk.confirmDeleteBook"
    />
    <Transition name="toast"
      ><div v-if="desk.toast" class="toast">
        <CheckCircle2 :size="18" />{{ desk.toast }}
      </div></Transition
    >
  </main>
</template>
<script setup lang="ts">
import { computed, reactive } from "vue";
import {
  CheckCircle2,
  CloudDownload,
  FolderOpen,
  Settings2,
} from "lucide-vue-next";
import AppSidebar from "./components/AppSidebar.vue";
import AuthModal from "./components/AuthModal.vue";
import AccountProfileModal from "./components/AccountProfileModal.vue";
import ChapterPickerModal from "./components/ChapterPickerModal.vue";
import ConfirmDeleteModal from "./components/ConfirmDeleteModal.vue";
import DownloadQueueModal from "./components/DownloadQueueModal.vue";
import SettingModal from "./components/SettingModal.vue";
import { useNovelDesk } from "./composables/useNovelDesk";
import BookshelfPage from "./pages/BookshelfPage.vue";
import DiscoverPage from "./pages/DiscoverPage.vue";
import LibraryPage from "./pages/LibraryPage.vue";
import LocalBookDetailPage from "./pages/LocalBookDetailPage.vue";
import LocalReaderPage from "./pages/LocalReaderPage.vue";
import LocalAudioPlayerPage from "./pages/LocalAudioPlayerPage.vue";

// reactive 会自动解包 composable 返回的 ref，模板中可直接读写 desk.xxx。
const desk = reactive(useNovelDesk());
const titleContent = computed(() => {
  switch (desk.active) {
    case "discover":
      return "发现想读的故事";
    case "bookshelf":
      return "我的 SF 书架";
    case "library":
      return "本地书库";
    case "libraryDetail":
      return desk.localBook?.name || "本地书籍";
    case "reader":
      return desk.localBook?.name || "本地阅读";
    case "audioPlayer":
      return desk.localBook?.name || "本地有声书";
    default:
      return "";
  }
});
</script>

<style scoped>
.app-shell {
  display: grid;
  grid-template-columns: 238px minmax(480px, 1fr);
  gap: 18px;
  height: 100vh;
  min-height: 0;
  padding: 18px;
  position: relative;
  overflow: hidden;
  align-items: start;
}

.app-shell::before,
.app-shell::after {
  position: fixed;
  border-radius: 50%;
  content: "";
  filter: blur(2px);
  opacity: 0.38;
  pointer-events: none;
}

.app-shell::before {
  top: 22%;
  left: -110px;
  width: 290px;
  height: 290px;
  background: #f8cfb7;
}
.app-shell::after {
  right: -90px;
  bottom: -60px;
  width: 260px;
  height: 260px;
  background: #f4b5a2;
}

.content {
  position: relative;
  z-index: 1;
  width: 100%;
  height: 100%;
  min-height: 0;
  max-width: 1200px;
  padding: 18px 42px 0px 2px;
  justify-self: center;
  overflow-x: hidden;
  overflow-y: auto;
  scrollbar-gutter: stable;
}
.immersive-active .content {
  grid-column: 1 / -1;
  max-width: 1200px;
  padding-top: 0;
}
.immersive-active .immersive-sidebar {
  display: none;
}
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 22px;
}
.topbar > div:first-child {
  min-width: 0;
}
.eyebrow {
  margin: 0 0 6px;
  color: #b08072;
  font: 10px monospace;
  letter-spacing: 0.1em;
}
.topbar h1 {
  margin: 0;
  color: #69453d;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 29px;
  line-height: 1.2;
}
.topbar-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}
.runtime-badge {
  max-width: 150px;
  overflow: hidden;
  padding: 5px 8px;
  border: 1px solid rgba(224, 176, 154, 0.72);
  border-radius: 6px;
  color: #94675c;
  background: rgba(255, 249, 245, 0.72);
  font: 10px monospace;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.icon-button {
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
.icon-button:hover {
  color: var(--theme-color-dark);
  background: #fff0e7;
}
.queue-fab {
  position: fixed;
  right: 28px;
  bottom: 28px;
  z-index: 8;
  display: grid;
  width: 56px;
  height: 56px;
  border: 0;
  border-radius: 18px;
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 12px 28px #a85d4250;
  place-items: center;
}
.queue-fab:hover {
  background: var(--theme-color-dark);
  transform: translateY(-2px);
}
.queue-count {
  position: absolute;
  top: -6px;
  right: -5px;
  display: grid;
  width: 27px;
  height: 27px;
  border-radius: 9px;
  color: #a85d42;
  background: #ffe0ce;
  font: 12px monospace;
  place-items: center;
}
.toast {
  position: fixed;
  bottom: 22px;
  left: 50%;
  z-index: 20;
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 11px 14px;
  border-radius: 12px;
  color: #fff;
  background: #654139;
  box-shadow: 0 10px 25px #6d403550;
  font-size: 13px;
  transform: translateX(-50%);
}
.toast-enter-active,
.toast-leave-active {
  transition: 0.2s;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translate(-50%, 8px);
}

@media (max-width: 1150px) {
  .app-shell {
    grid-template-columns: 215px minmax(0, 1fr);
  }
}
@media (max-width: 760px) {
  .app-shell {
    display: block;
    min-height: 100vh;
    padding: 8px 12px calc(78px + env(safe-area-inset-bottom));
    overflow: visible;
    overflow-y: auto;
    scrollbar-width: none;
  }
  .content {
    height: auto;
    overflow: visible;
    padding: 10px 0 0;
  }
  .immersive-active {
    padding-bottom: 16px;
  }
  .immersive-active .content {
    padding-top: 0;
  }
  .immersive-active .queue-fab {
    display: none;
  }
  .topbar {
    display: none;
  }
  .queue-fab {
    right: 18px;
    bottom: calc(70px + env(safe-area-inset-bottom));
    width: 52px;
    height: 52px;
    border-radius: 16px;
  }
  .toast {
    bottom: calc(78px + env(safe-area-inset-bottom));
    max-width: calc(100vw - 32px);
  }
}
</style>
