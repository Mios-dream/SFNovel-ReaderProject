import { onBackButtonPress } from "@tauri-apps/api/app";
import { onBeforeUnmount, onMounted } from "vue";
import type { AuthDesk } from "./domains/auth";
import type { JobsDesk } from "./domains/jobs";
import type { LibraryDesk } from "./domains/library";
import type { PickerDesk } from "./domains/picker";
import type { RemoteDesk } from "./domains/remote";
import type { SettingsDesk } from "./domains/settings";
import type { SigninDesk } from "./domains/signin";
import type { UpdaterDesk } from "./domains/updater";
import type { WorkspaceDesk } from "./domains/workspace";
import { isReaderView } from "./navigation";

type DeskLifecycleOptions = {
  auth: AuthDesk;
  jobs: JobsDesk;
  library: LibraryDesk;
  picker: PickerDesk;
  remote: RemoteDesk;
  settings: SettingsDesk;
  signin: SigninDesk;
  updater: UpdaterDesk;
  workspace: WorkspaceDesk;
};

/**
 * 应用级副作用。
 *
 * 启动初始化、返回键逐层回退、禁止复制、回到前台刷新书库都跨越几乎所有域，属于
 * 「应用装配」而非某个页面流程，因此不放进 `workspace`——后者只回答页面之间
 * 怎么走。
 *
 * 必须在组件 setup 内调用（`provider` 在 `App.vue` 的 setup 中执行）。
 */
export function registerDeskLifecycle({
  auth,
  jobs,
  library,
  picker,
  remote,
  settings,
  signin,
  updater,
  workspace,
}: DeskLifecycleOptions) {
  const { navigate, readerSource, currentView, backFromOnlineReading } =
    workspace;

  /**
   * Android 返回键：先逐层关闭浮层，再按路由回退一层。
   *
   * 浮层顺序即用户感知的层级：章节选择 → 下载队列 → 签到 → 登录 → 账户 → 更新 →
   * 设置 → 删除确认。任何一层命中后立即返回，不继续回退页面。
   */
  function handleBackNavigation() {
    if (picker.chapterModalOpen.value) {
      picker.chapterModalOpen.value = false;
      return;
    }
    if (jobs.queueOpen.value) {
      jobs.queueOpen.value = false;
      return;
    }
    if (auth.signinOpen.value) {
      auth.signinOpen.value = false;
      return;
    }
    if (auth.credentialsOpen.value) {
      auth.credentialsOpen.value = false;
      return;
    }
    if (auth.accountOpen.value) {
      auth.accountOpen.value = false;
      return;
    }
    if (updater.modalOpen.value) {
      updater.closeModal();
      return;
    }
    if (settings.settingsOpen.value) {
      settings.settingsOpen.value = false;
      return;
    }
    if (library.confirmBook.value) {
      library.confirmBook.value = undefined;
      return;
    }
    const view = currentView();
    if (isReaderView(view)) {
      if (readerSource.value === "online") backFromOnlineReading();
      else void navigate("libraryDetail");
      return;
    }
    if (view === "libraryDetail") {
      void navigate("library");
      return;
    }
    if (view === "remoteDetail") remote.closeRemoteBookDetail();
  }

  function blockCopy(event: ClipboardEvent) {
    event.preventDefault();
  }

  /** 从系统文件管理器返回时书库可能已变化。 */
  function refreshLibraryAfterReturningToApp() {
    if (document.visibilityState === "visible" && currentView() === "library")
      void library.refreshLibrary();
  }

  let stopBackListener: { unregister: () => Promise<void> } | undefined;
  let disposed = false;

  onMounted(() => {
    void onBackButtonPress(() => handleBackNavigation())
      .then((unlisten) => {
        if (disposed) void unlisten.unregister();
        else stopBackListener = unlisten;
      })
      .catch(() => {
        // Android 应用插件在浏览器与桌面端不可用。
      });
    void auth.refreshAuthStatus().then(() => auth.loadAccountProfile());
    void signin.announceAutoSignin();
    void updater.loadCurrentVersion();
    void settings
      .loadSettings()
      .then((loaded) => {
        if (loaded.updates.autoCheck) void updater.checkForUpdate(false);
      })
      .catch(() => {
        // 设置读取失败不阻塞启动。
      });
    document.addEventListener("copy", blockCopy);
    document.addEventListener(
      "visibilitychange",
      refreshLibraryAfterReturningToApp,
    );
  });

  onBeforeUnmount(() => {
    disposed = true;
    void stopBackListener?.unregister();
    document.removeEventListener("copy", blockCopy);
    document.removeEventListener(
      "visibilitychange",
      refreshLibraryAfterReturningToApp,
    );
  });
}
