import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { AuthStatus, UserProfile } from "../types";
import { nativeErrorMessage, type Notify } from "./deskShared";

export function useDeskAuth(notify: Notify) {
  const auth = ref<AuthStatus>({ authenticated: false, appAuthenticated: false, webAuthenticated: false });
  const credentialsOpen = ref(false);
  const accountOpen = ref(false);
  const accountProfile = ref<UserProfile>();
  const accountProfileLoading = ref(false);
  const accountProfileError = ref("");
  const loginBusy = ref(false);

  async function refreshAuthStatus() {
    try {
      auth.value = await invoke<AuthStatus>("auth_status");
    } catch {
      /* Native runtime may be restarting during development. */
    }
  }

  async function loadAccountProfile() {
    if (!auth.value.webAuthenticated) {
      accountProfile.value = undefined;
      accountProfileError.value = "";
      return false;
    }
    accountProfileLoading.value = true;
    accountProfileError.value = "";
    try {
      const profile = await invoke<UserProfile>("get_web_user_profile");
      accountProfile.value = profile;
      auth.value = { ...auth.value, userName: profile.nickName };
      if (auth.value.appAuthenticated) {
        try {
          const appProfile = await invoke<UserProfile>("get_user_profile");
          accountProfile.value = {
            ...profile,
            accountId: profile.accountId || appProfile.accountId,
            appDetailsAvailable: appProfile.appDetailsAvailable,
            webDetailsAvailable: profile.webDetailsAvailable,
            vipDetailsAvailable:
              profile.vipDetailsAvailable || appProfile.vipDetailsAvailable,
            vipSystem: profile.vipDetailsAvailable
              ? profile.vipSystem
              : appProfile.vipSystem,
            welfareCoin: appProfile.welfareCoin,
            fireMoneyRemain: appProfile.fireMoneyRemain,
            couponsRemain: appProfile.couponsRemain,
            monthlyTicket: profile.monthlyTicket,
            vipLevel: profile.vipDetailsAvailable
              ? profile.vipLevel
              : appProfile.vipLevel,
            vipName: profile.vipName,
          };
        } catch (error) {
          accountProfileError.value = nativeErrorMessage(
            error,
            "App 余额资料暂不可用",
          );
        }
      }
      return true;
    } catch (error) {
      accountProfile.value = undefined;
      accountProfileError.value = nativeErrorMessage(error, "读取账户资料失败");
      return false;
    } finally {
      accountProfileLoading.value = false;
    }
  }

  async function login(username: string, password: string) {
    loginBusy.value = true;
    try {
      auth.value = await invoke<AuthStatus>("login_with_password", {
        username,
        password,
      });
      await loadAccountProfile();
      notify(
        auth.value.webAuthenticated
          ? "App 高级能力已连接"
          : "App 高级能力已连接，可选择继续连接网站功能",
      );
    } catch (error) {
      notify(nativeErrorMessage(error, "SF 账号密码登录失败"));
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
      if (!(await loadAccountProfile())) {
        notify(accountProfileError.value || "网页登录会话已失效");
        return;
      }
      notify(
        auth.value.appAuthenticated
          ? "网站功能已连接"
          : "网站功能已连接，可选择继续连接 App 高级能力",
      );
    } catch (error) {
      notify(nativeErrorMessage(error, "无法打开官方登录窗口"));
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
      accountProfile.value = undefined;
      accountProfileError.value = "";
      notify("已清除本地登录会话");
    } catch {
      notify("退出登录失败");
    }
  }

  async function logoutApp() {
    try {
      auth.value = await invoke<AuthStatus>("logout_app_session");
      if (auth.value.webAuthenticated) await loadAccountProfile();
      else {
        accountProfile.value = undefined;
        accountProfileError.value = "";
        accountOpen.value = false;
      }
      notify("App 登录凭证已退出，网站功能不受影响");
    } catch (error) {
      notify(nativeErrorMessage(error, "退出 App 登录失败"));
    }
  }

  async function logoutWeb() {
    try {
      auth.value = await invoke<AuthStatus>("logout_web_session");
      accountProfile.value = undefined;
      accountProfileError.value = "";
      accountOpen.value = false;
      notify("网站登录凭证已退出，App 功能不受影响");
    } catch (error) {
      notify(nativeErrorMessage(error, "退出网站登录失败"));
    }
  }

  async function openAccount() {
    if (!auth.value.webAuthenticated) {
      credentialsOpen.value = true;
      return;
    }
    accountOpen.value = true;
    await loadAccountProfile();
  }

  function manageConnections() {
    accountOpen.value = false;
    credentialsOpen.value = true;
  }

  return {
    auth,
    credentialsOpen,
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
  };
}
