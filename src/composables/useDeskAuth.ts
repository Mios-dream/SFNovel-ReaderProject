import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { AuthStatus, UserProfile } from "../types";
import { nativeErrorMessage, type Notify } from "./deskShared";

const ACCOUNT_PROFILE_CACHE_MS = 5 * 60 * 1000;

export function useDeskAuth(notify: Notify) {
  const auth = ref<AuthStatus>({ authenticated: false, appAuthenticated: false, webAuthenticated: false });
  const credentialsOpen = ref(false);
  const credentialsTab = ref<"official" | "password">("official");
  const accountOpen = ref(false);
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
        accountProfileError.value = nativeErrorMessage(error, "读取账户资料失败");
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
      auth.value = { authenticated: false, appAuthenticated: false, webAuthenticated: false };
      accountOpen.value = false;
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
      if (auth.value.authenticated) {
        await loadAccountProfile(true);
      } else {
        clearAccountProfileCache();
        accountOpen.value = false;
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

  function openCredentials(tab: "official" | "password" = "official") {
    credentialsTab.value = tab;
    credentialsOpen.value = true;
  }

  function openAppCredentials() {
    openCredentials("password");
  }

  return {
    auth,
    credentialsOpen,
    credentialsTab,
    accountOpen,
    accountProfile,
    accountProfileLoading,
    accountProfileError,
    loginBusy,
    refreshAuthStatus,
    loadAccountProfile,
    login,
    browserLogin,
    logout,
    logoutApp,
    logoutWeb,
    openAccount,
    manageConnections,
    openCredentials,
    openAppCredentials,
  };
}
