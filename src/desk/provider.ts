import { provide } from "vue";
import { showToast } from "../composables/useToast";
import { createAuthDesk } from "./domains/auth";
import { createBookshelfDesk } from "./domains/bookshelf";
import { createJobsDesk } from "./domains/jobs";
import { createLibraryDesk } from "./domains/library";
import { createOnlineDesk } from "./domains/online";
import { createPickerDesk } from "./domains/picker";
import { createSearchDesk } from "./domains/search";
import { createSettingsDesk } from "./domains/settings";
import { createSigninDesk } from "./domains/signin";
import { createUpdaterDesk } from "./domains/updater";
import {
  authKey,
  bookshelfKey,
  jobsKey,
  libraryKey,
  onlineKey,
  pickerKey,
  remoteKey,
  searchKey,
  settingsKey,
  signinKey,
  updaterKey,
  workspaceKey,
} from "./keys";
import { registerDeskLifecycle } from "./lifecycle";
import { createRemoteDesk } from "./domains/remote";
import { createWorkspaceDesk } from "./domains/workspace";

/**
 * 组合根：创建各域、注入跨域依赖、把上下文挂到注入键上。
 *
 * 这里只允许出现装配代码——任何业务判断都应该下沉到对应域。
 * 必须在组件 setup 中调用一次（`App.vue`），域内的 `onMounted` 会注册到该组件。
 */
export function useDeskProvider() {
  const notify = showToast;

  const auth = createAuthDesk(notify);
  const settings = createSettingsDesk(notify);
  const signin = createSigninDesk({ settings, notify });
  const updater = createUpdaterDesk({
    notify,
    closeSettings: () => {
      settings.settingsOpen.value = false;
    },
  });
  const library = createLibraryDesk({ notify });
  const jobs = createJobsDesk({
    notify,
    refreshLibrary: library.refreshLibrary,
    refreshLocalBook: library.refreshLocalBook,
  });
  const search = createSearchDesk({
    refreshAuthStatus: auth.refreshAuthStatus,
    notify,
  });
  const bookshelf = createBookshelfDesk({
    auth: auth.auth,
    refreshAuthStatus: auth.refreshAuthStatus,
    requestCredentials: () => {
      auth.openCredentials();
    },
    notify,
  });
  const online = createOnlineDesk({ notify });
  const remote = createRemoteDesk({
    auth: auth.auth,
    notify,
    refreshBookshelf: bookshelf.refreshBookshelf,
    resetOnlineSession: online.resetOnlineReadingSession,
  });
  const picker = createPickerDesk({
    auth: auth.auth,
    notify,
    loadCatalog: (novel, mode) =>
      online.pickerCatalog(novel, mode, remote.isSessionActive()),
    loadDetail: remote.resolveDetail,
    enqueueDownload: async (mode, novel, chapterIds) => {
      await jobs.createDownload(mode, novel, chapterIds);
    },
  });
  const workspace = createWorkspaceDesk({
    library,
    bookshelf,
    online,
    picker,
    remote,
  });

  registerDeskLifecycle({
    auth,
    jobs,
    library,
    picker,
    remote,
    settings,
    updater,
    workspace,
  });

  provide(authKey, auth);
  provide(libraryKey, library);
  provide(jobsKey, jobs);
  provide(bookshelfKey, bookshelf);
  provide(searchKey, search);
  provide(settingsKey, settings);
  provide(signinKey, signin);
  provide(updaterKey, updater);
  provide(onlineKey, online);
  provide(pickerKey, picker);
  provide(remoteKey, remote);
  provide(workspaceKey, workspace);
}
