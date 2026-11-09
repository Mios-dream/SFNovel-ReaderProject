import { onBackButtonPress } from "@tauri-apps/api/app";
import { onBeforeUnmount, onMounted } from "vue";
import type { AuthDesk } from "./domains/auth";
import type { LibraryDesk } from "./domains/library";
import type { SettingsDesk } from "./domains/settings";
import type { SigninDesk } from "./domains/signin";
import type { UpdaterDesk } from "./domains/updater";
import type { WorkspaceDesk } from "./domains/workspace";
import { dismissTopLayer } from "./backLayers";

type DeskLifecycleOptions = {
  auth: AuthDesk;
  library: LibraryDesk;
  settings: SettingsDesk;
  signin: SigninDesk;
  updater: UpdaterDesk;
  workspace: WorkspaceDesk;
};

/**
 * 应用级副作用。
 *
 * 启动初始化、返回事件接管、禁止复制、回到前台刷新书库都跨越几乎所有域，属于
 * 「应用装配」而非某个页面流程，因此不放进 `workspace`——后者只回答页面之间
 * 怎么走。
 *
 * 必须在组件 setup 内调用（`provider` 在 `App.vue` 的 setup 中执行）。
 */
export function registerDeskLifecycle({
  auth,
  library,
  settings,
  signin,
  updater,
  workspace,
}: DeskLifecycleOptions) {
  const { back, currentView } = workspace;

  /**
   * 返回事件只有两个入口，都汇到同一条栈：
   *
   * - Android 返回键 → `back()`：先弹最上层浮层，没有浮层再按路由表回退一层。
   * - 桌面 Esc → `dismissTopLayer()`：只弹浮层，不回退页面。
   *
   * 浮层由 `ModalShell` 与页内面板在打开时自行注册，顺序即叠放顺序；路由层由
   * `workspace` 的返回表声明。装配层不再维护任何层级清单，也不再判断当前视图。
   */
  function handleEscape(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    if (dismissTopLayer()) event.preventDefault();
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
    void onBackButtonPress(() => back())
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
    document.addEventListener("keydown", handleEscape);
    document.addEventListener(
      "visibilitychange",
      refreshLibraryAfterReturningToApp,
    );
  });

  onBeforeUnmount(() => {
    disposed = true;
    void stopBackListener?.unregister();
    document.removeEventListener("copy", blockCopy);
    document.removeEventListener("keydown", handleEscape);
    document.removeEventListener(
      "visibilitychange",
      refreshLibraryAfterReturningToApp,
    );
  });
}
