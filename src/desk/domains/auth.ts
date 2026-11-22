import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type {
  AuthStatus,
  CredentialKind,
  LoginCredentials,
  UserProfile,
} from "../../types";
import { nativeErrorMessage } from "../../utils/native";
import {
  credentialKind,
  formatLoginCredentials,
  parseLoginCredentials,
} from "../../utils/credentials";
import type { Notify } from "../../composables/useToast";

const ACCOUNT_PROFILE_CACHE_MS = 5 * 60 * 1000;

/**
 * 写入系统剪贴板。
 *
 * WebView 的异步剪贴板在部分 Android 版本不可用，失败时交给原生实现。
 */
async function writeClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    await invoke("copy_to_clipboard", { text });
  }
}

export function createAuthDesk(notify: Notify) {
  const auth = ref<AuthStatus>({
    authenticated: false,
    appAuthenticated: false,
    webAuthenticated: false,
  });
  const credentialsOpen = ref(false);
  const credentialsTab = ref<CredentialKind>("app");
  const accountOpen = ref(false);
  const signinOpen = ref(false);
  const accountProfile = ref<UserProfile>();
  const accountProfileLoading = ref(false);
  const accountProfileError = ref("");
  const loginBusy = ref(false);
  let accountProfileLoadedAt = 0;
  let accountProfileRequest: Promise<boolean> | undefined;

  function clearAccountProfileCache() {
    accountProfile.value = undefined;
    accountProfileError.value = "";
    accountProfileLoadedAt = 0;
  }

  async function refreshAuthStatus() {
    try {
      auth.value = await invoke<AuthStatus>("auth_status");
    } catch {
      /* Native runtime may be restarting during development. */
    }
  }

  async function loadAccountProfile(force = false) {
    if (!auth.value.authenticated) {
      clearAccountProfileCache();
      return false;
    }
    if (
      !force &&
      accountProfile.value &&
      Date.now() - accountProfileLoadedAt < ACCOUNT_PROFILE_CACHE_MS
    ) {
      return true;
    }
    if (accountProfileRequest) return accountProfileRequest;
    const request = (async () => {
      accountProfileLoading.value = true;
      accountProfileError.value = "";
      try {
        const profile = await invoke<UserProfile>("get_account_profile");
        accountProfile.value = profile;
        accountProfileLoadedAt = Date.now();
        auth.value = { ...auth.value, userName: profile.nickName };
        return true;
      } catch (error) {
        clearAccountProfileCache();
        accountProfileError.value = nativeErrorMessage(
          error,
          "读取账户资料失败",
        );
        await refreshAuthStatus();
        return false;
      } finally {
        accountProfileLoading.value = false;
      }
    })();
    accountProfileRequest = request;
    try {
      return await request;
    } finally {
      if (accountProfileRequest === request) accountProfileRequest = undefined;
    }
  }

  async function login(username: string, password: string) {
    loginBusy.value = true;
    try {
      auth.value = await invoke<AuthStatus>("login_with_password", {
        username,
        password,
      });
      await loadAccountProfile(true);
      notify("App 登录已连接");
    } catch (error) {
      notify(nativeErrorMessage(error, "SF 账号密码登录失败"), "error");
    } finally {
      loginBusy.value = false;
    }
  }

  /**
   * 用其他设备复制的凭据建立会话。
   *
   * @param payload 复制来的凭据 JSON；类型由 Cookie 里的会话键决定，粘贴在哪个
   * 页签都一样。
   *
   * 登录成功后把页签挪到凭据真正落到的那一侧：用户可能在「网站登录」里粘了一份
   * App 凭据，停在原页签只会看到「未连接」。
   */
  async function loginWithCookie(payload: string) {
    const { cookie, deviceId } = parseLoginCredentials(payload);
    if (!cookie) {
      notify("请粘贴导出的登录凭据 JSON");
      return;
    }
    const kind = credentialKind(cookie);
    if (!kind) {
      notify("Cookie错误，无法判断登录类型", "warning");
      return;
    }
    loginBusy.value = true;
    try {
      auth.value = await invoke<AuthStatus>("login_with_cookie", {
        cookie,
        deviceId,
      });
      if (!(await loadAccountProfile(true))) {
        notify(
          accountProfileError.value || "登录凭据已失效，请重新获取",
          "error",
        );
        return;
      }
      credentialsTab.value = kind;
      notify(kind === "web" ? "网站登录已连接" : "App 登录已连接");
    } catch (error) {
      notify(nativeErrorMessage(error, "凭据登录失败"), "error");
    } finally {
      loginBusy.value = false;
    }
  }

  /**
   * 消费电脑端脚本投递的登录凭据。
   *
   * 脚本把凭据写进应用专属目录后冷启动应用，这里解析一次并建立登录；
   * 文件无论成败都会被原生侧删除，因此只在启动时调用。
   */
  async function consumeDeliveredLogin() {
    try {
      if (await invoke<boolean>("consume_delivered_login")) {
        notify("已从官方 App 同步登录");
      }
    } catch (error) {
      notify(nativeErrorMessage(error, "同步登录失败"), "error");
    }
  }

  /**
   * 复制当前会话的登录凭据。
   *
   * @param kind 要导出的会话：网站凭据只含网页 Cookie，App 凭据带配套设备编号。
   *
   * Cookie 与设备编号必须配套，所以合并成一段 JSON 一次复制：另一台设备粘贴一次
   * 即可，字段边界明确，也不会被 Cookie 里的空格或分号切错。
   */
  async function copyCredentials(kind: CredentialKind) {
    try {
      const credentials = await invoke<LoginCredentials>(
        "get_login_credentials",
        { kind },
      );
      await writeClipboard(formatLoginCredentials(credentials));
      notify(kind === "web" ? "网站登录凭据已复制" : "App 登录凭据已复制");
    } catch (error) {
      notify(nativeErrorMessage(error, "复制失败"), "error");
    }
  }

  async function browserLogin() {
    loginBusy.value = true;
    try {
      await invoke<void>("start_official_login");
      const result = await invoke<AuthStatus>("auth_status");
      if (!result.authenticated) {
        notify("未检测到官方登录会话");
        return;
      }
      auth.value = result;
      if (!(await loadAccountProfile(true))) {
        notify(accountProfileError.value || "网页登录会话已失效", "error");
        return;
      }
      notify("网站登录已连接");
    } catch (error) {
      notify(nativeErrorMessage(error, "无法打开官方登录窗口"), "error");
    } finally {
      loginBusy.value = false;
    }
  }

  async function logout() {
    try {
      await invoke<void>("logout");
      auth.value = {
        authenticated: false,
        appAuthenticated: false,
        webAuthenticated: false,
      };
      accountOpen.value = false;
      signinOpen.value = false;
      credentialsOpen.value = false;
      clearAccountProfileCache();
      notify("已清除本地登录会话");
    } catch {
      notify("退出登录失败", "error");
    }
  }

  async function logoutApp() {
    try {
      auth.value = await invoke<AuthStatus>("logout_app_session");
      signinOpen.value = false;
      if (auth.value.authenticated) {
        await loadAccountProfile(true);
      } else {
        clearAccountProfileCache();
        accountOpen.value = false;
        signinOpen.value = false;
      }
      notify("App 登录已退出，网站登录不受影响");
    } catch (error) {
      notify(nativeErrorMessage(error, "退出 App 登录失败"), "error");
    }
  }

  async function logoutWeb() {
    try {
      auth.value = await invoke<AuthStatus>("logout_web_session");
      if (auth.value.authenticated) {
        await loadAccountProfile(true);
      } else {
        clearAccountProfileCache();
        accountOpen.value = false;
      }
      notify("网站登录已退出，App 登录不受影响");
    } catch (error) {
      notify(nativeErrorMessage(error, "退出网站登录失败"), "error");
    }
  }

  async function openAccount() {
    if (!auth.value.authenticated) {
      openCredentials();
      return;
    }
    accountOpen.value = true;
    await loadAccountProfile();
  }

  function manageConnections() {
    accountOpen.value = false;
    openCredentials();
  }

  function openSignin() {
    if (!auth.value.appAuthenticated) return;
    accountOpen.value = false;
    signinOpen.value = true;
  }

  function openCredentials(tab: CredentialKind = "app") {
    signinOpen.value = false;
    credentialsTab.value = tab;
    credentialsOpen.value = true;
  }

  function openAppCredentials() {
    openCredentials("app");
  }

  /**
   * 音频播放的登录门禁：App 凭证缺失时提示并拒绝播放。
   * @returns 是否允许继续播放。
   */
  function requireAudioPlaybackAccess() {
    if (auth.value.appAuthenticated) return true;
    notify("音频地址为空，请登录 App 凭证后播放", "warning");
    return false;
  }

  return {
    auth,
    credentialsOpen,
    credentialsTab,
    accountOpen,
    signinOpen,
    accountProfile,
    accountProfileLoading,
    accountProfileError,
    loginBusy,
    refreshAuthStatus,
    loadAccountProfile,
    login,
    loginWithCookie,
    consumeDeliveredLogin,
    copyCredentials,
    browserLogin,
    logout,
    logoutApp,
    logoutWeb,
    openAccount,
    openSignin,
    manageConnections,
    openCredentials,
    openAppCredentials,
    requireAudioPlaybackAccess,
  };
}

/** 该域对外暴露的完整上下文；页面只应通过 `useAuth` 取用。 */
export type AuthDesk = ReturnType<typeof createAuthDesk>;
