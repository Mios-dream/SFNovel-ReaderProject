import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { AuthStatus, UserProfile } from "../types";
import { nativeErrorMessage, type Notify } from "./deskShared";

export function useDeskAuth(notify: Notify) {
  const auth = ref<AuthStatus>({ authenticated: false });
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
    if (!auth.value.authenticated) return;
    accountProfileLoading.value = true;
    accountProfileError.value = "";
    try {
      const profile = await invoke<UserProfile>("get_user_profile");
      accountProfile.value = profile;
      auth.value = { ...auth.value, userName: profile.nickName };
    } catch (error) {
      accountProfileError.value = nativeErrorMessage(error, "读取账户资料失败");
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
      credentialsOpen.value = false;
      await loadAccountProfile();
      notify("SF 账号已登录到当前会话");
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
      credentialsOpen.value = false;
      await loadAccountProfile();
      notify("SF 账号已登录到当前会话");
    } catch (error) {
      notify(nativeErrorMessage(error, "无法打开官方登录窗口"));
    } finally {
      loginBusy.value = false;
    }
  }

  async function logout() {
    try {
      await invoke<void>("logout");
      auth.value = { authenticated: false };
      accountOpen.value = false;
      credentialsOpen.value = false;
      accountProfile.value = undefined;
      accountProfileError.value = "";
      notify("已清除本地登录会话");
    } catch {
      notify("退出登录失败");
    }
  }

  async function openAccount() {
    if (!auth.value.authenticated) {
      credentialsOpen.value = true;
      return;
    }
    accountOpen.value = true;
    await loadAccountProfile();
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
    openAccount,
  };
}
