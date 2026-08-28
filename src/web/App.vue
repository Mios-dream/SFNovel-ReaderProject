<script setup lang="ts">
import { reactive } from "vue";
import { CheckCircle2, CloudDownload, FolderOpen } from "lucide-vue-next";
import AppSidebar from "./components/AppSidebar.vue";
import AuthModal from "./components/AuthModal.vue";
import ChapterPickerModal from "./components/ChapterPickerModal.vue";
import ConfirmDeleteModal from "./components/ConfirmDeleteModal.vue";
import DownloadQueueModal from "./components/DownloadQueueModal.vue";
import { useNovelDesk } from "./composables/useNovelDesk";
import BookshelfPage from "./pages/BookshelfPage.vue";
import DiscoverPage from "./pages/DiscoverPage.vue";
import LibraryPage from "./pages/LibraryPage.vue";

// A reactive wrapper unwraps the refs returned by the composable when accessed
// from the template, including when they are passed to child components.
const desk = reactive(useNovelDesk());
</script>

<template>
  <main class="app-shell">
    <AppSidebar :active="desk.active" :bookshelf-count="desk.bookshelf.length" :library-count="desk.library.length" :auth="desk.auth" @navigate="desk.navigate" @login="desk.credentialsOpen = true" />

    <section class="content">
      <header class="topbar">
        <div><p class="eyebrow">PERSONAL NOVEL DESK</p><h1>{{ desk.active === 'discover' ? '发现想读的故事' : desk.active === 'bookshelf' ? '我的 SF 书架' : '你的本地书库' }}</h1></div>
        <button class="icon-button" title="打开输出目录" @click="desk.navigate('library')"><FolderOpen :size="20" /></button>
      </header>

      <DiscoverPage v-if="desk.active === 'discover'" v-model:query="desk.query" :results="desk.results" :loading="desk.loading" :searched="desk.searched" :format-date="desk.formatDate" @search="desk.search" @select="desk.openChapterPicker" />
      <BookshelfPage v-else-if="desk.active === 'bookshelf'" :novels="desk.bookshelf" :visible-novels="desk.pagedBookshelf" :loading="desk.bookshelfLoading" :categories="desk.bookshelfCategories" :active-category="desk.bookshelfCategory" :page="desk.bookshelfPage" :total-pages="desk.bookshelfTotalPages" :filtered-count="desk.filteredBookshelf.length" :format-date="desk.formatDate" @refresh="desk.refreshBookshelf" @category="desk.selectBookshelfCategory" @page="desk.setBookshelfPage" @select="desk.openChapterPicker" />
      <LibraryPage v-else v-model:managing="desk.libraryManaging" :books="desk.library" :jobs="desk.libraryJobs" :format-date="desk.formatDate" @open="desk.openLibraryBook" @remove="desk.deleteBook" @pause="desk.pauseJob" @resume="desk.resumeJob" @discover="desk.navigate('discover')" />
    </section>

    <button class="queue-fab" title="打开下载列表" @click="desk.queueOpen = true"><CloudDownload :size="22" /><span v-if="desk.runningJobs.length" class="queue-count">{{ desk.runningJobs.length }}</span></button>

    <ChapterPickerModal :open="desk.chapterModalOpen" :novel="desk.chapterNovel" :mode="desk.chapterMode" :loading="desk.chapterLoading" :has-audio="desk.chapterHasAudio" :volumes="desk.chapterVolumes" :audio-chapters="desk.audioChapters" :selected-ids="desk.selectedChapterIds" :format-date="desk.formatDate" @close="desk.chapterModalOpen = false" @update:selected-ids="desk.selectedChapterIds = $event" @change-mode="desk.changeChapterMode" @toggle-all="desk.toggleAllChapters" @confirm="desk.confirmChapterDownload" />
    <DownloadQueueModal :open="desk.queueOpen" :jobs="desk.jobs" @close="desk.queueOpen = false" @pause="desk.pauseJob" @resume="desk.resumeJob" @remove="desk.deleteJob" />
    <AuthModal :open="desk.credentialsOpen" :auth="desk.auth" :busy="desk.loginBusy" @close="desk.credentialsOpen = false" @login="desk.login" @logout="desk.logout" />
    <ConfirmDeleteModal :book="desk.confirmBook" @close="desk.confirmBook = undefined" @confirm="desk.confirmDeleteBook" />
    <Transition name="toast"><div v-if="desk.toast" class="toast"><CheckCircle2 :size="18" />{{ desk.toast }}</div></Transition>
  </main>
</template>

<style scoped>
.app-shell {
  display: grid;
  grid-template-columns: 238px minmax(480px, 1fr);
  gap: 18px;
  min-height: 100vh;
  padding: 18px;
  position: relative;
  overflow: hidden;
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

.app-shell::before { top: 22%; left: -110px; width: 290px; height: 290px; background: #f8cfb7; }
.app-shell::after { right: -90px; bottom: -60px; width: 260px; height: 260px; background: #f4b5a2; }

.content { position: relative; z-index: 1; width: 100%; max-width: 1040px; padding: 18px 2px 42px; justify-self: center; }
.topbar { display: flex; align-items: center; justify-content: space-between; margin-bottom: 22px; }
.eyebrow { margin: 0 0 6px; color: #b08072; font: 10px monospace; letter-spacing: 0.1em; }
.topbar h1 { margin: 0; color: #69453d; font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 29px; line-height: 1.2; }
.icon-button { display: grid; width: 40px; height: 40px; border: 0; border-radius: 13px; color: #a17367; background: rgba(255, 255, 255, 0.7); place-items: center; transition: 0.2s; }
.icon-button:hover { color: var(--theme-color-dark); background: #fff0e7; }
.queue-fab { position: fixed; right: 28px; bottom: 28px; z-index: 8; display: grid; width: 56px; height: 56px; border: 0; border-radius: 18px; color: #fff; background: var(--theme-color); box-shadow: 0 12px 28px #a85d4250; place-items: center; }
.queue-fab:hover { background: var(--theme-color-dark); transform: translateY(-2px); }
.queue-count { position: absolute; top: -6px; right: -5px; display: grid; width: 27px; height: 27px; border-radius: 9px; color: #a85d42; background: #ffe0ce; font: 12px monospace; place-items: center; }
.toast { position: fixed; bottom: 22px; left: 50%; z-index: 20; display: flex; align-items: center; gap: 7px; padding: 11px 14px; border-radius: 12px; color: #fff; background: #654139; box-shadow: 0 10px 25px #6d403550; font-size: 13px; transform: translateX(-50%); }
.toast-enter-active, .toast-leave-active { transition: 0.2s; }
.toast-enter-from, .toast-leave-to { opacity: 0; transform: translate(-50%, 8px); }

@media (max-width: 1150px) { .app-shell { grid-template-columns: 215px minmax(0, 1fr); } }
@media (max-width: 760px) {
  .app-shell { display: block; padding: 10px; }
  .content { padding: 12px 2px; }
  .topbar { margin: 6px 3px 17px; }
  .topbar h1 { font-size: 24px; }
  .queue-fab { right: 16px; bottom: 16px; }
}
</style>
