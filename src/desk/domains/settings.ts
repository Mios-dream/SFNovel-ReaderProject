import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { AppSettings, BackgroundPermissions } from "../../types";
import type { Notify } from "../../composables/useToast";
import { nativeErrorMessage } from "../../utils/native";

/** 后台运行权限里可以主动申请的两项。 */
export type BackgroundPending = "battery" | "notification" | null;

/** 后台运行权限只存在于 Android；其他平台不展示相关入口。 */
const isAndroid = /Android/i.test(navigator.userAgent);

/**
 * 后端返回设置之前的占位值。
 *
 * 只用于首帧渲染，任何写入都以后端回传的完整对象为准；因此这里不维护「用户
 * 已修改」的中间态，避免出现第二份真相。
 */
function defaultSettings(): AppSettings {
  return {
    download: {
      requestIntervalMs: 500,
      maxConcurrentDownloads: 1,
      ocrProcessFilesEnabled: false,
    },
    storage: {
      useExternalStorage: false,
    },
    desktop: {
      closeToTray: true,
      autostart: false,
    },
    signin: {
      autoSigninEnabled: false,
    },
    updates: {
      autoCheck: true,
    },
  };
}

export function createSettingsDesk(notify: Notify) {
  const settingsOpen = ref(false);
  const settings = ref<AppSettings>(defaultSettings());
  const settingsSaving = ref(false);
  const contentDictionarySize = ref(0);
  const contentDictionaryUpdating = ref(false);
  const contentDictionaryChapterId = ref(8436696);
  /** Android 后台运行权限的最新读取结果；未读取过时为空。 */
  const backgroundPermissions = ref<BackgroundPermissions>();
  /** 正在请求哪一项权限；只让被点的那一行进入处理中，另一行仍可点。 */
  const backgroundPending = ref<BackgroundPending>(null);

  /** 是否已经向后端读取过；避免重复请求，也让其他域可以按需等待。 */
  let loaded = false;

  async function loadSettings() {
    const value = await invoke<AppSettings>("get_settings");
    settings.value = value;
    loaded = true;
    return value;
  }

  /** 供其他域按需取用：未加载时先读取，已加载时直接返回当前值。 */
  async function ensureSettings() {
    if (!loaded) await loadSettings();
    return settings.value;
  }

  /** 写回后端并同步本地副本；调用方负责错误提示与后续动作。 */
  async function persist(value: AppSettings) {
    const saved = await invoke<AppSettings>("save_settings", {
      settings: value,
    });
    settings.value = saved;
    loaded = true;
    return saved;
  }

  async function openSettings() {
    settingsOpen.value = true;
    try {
      await Promise.all([loadSettings(), loadBackgroundPermissions()]);
      const dictionary = await invoke<{ size: number }>("get_content_dictionary");
      contentDictionarySize.value = dictionary.size;
    } catch (error) {
      notify(nativeErrorMessage(error, "读取设置失败"), "error");
    }
  }

  /**
   * 读取后台运行权限。
   *
   * 用户可能在系统页面里改完就返回，所以每次回到应用都要重读；读取失败按未知处理，
   * 不打扰用户——这些状态只影响提示，不影响签到本身。
   */
  async function loadBackgroundPermissions() {
    if (!isAndroid) return;
    try {
      backgroundPermissions.value = await invoke<BackgroundPermissions>(
        "read_background_permissions",
      );
    } catch {
      // 状态是附加信息，读不到就不展示结论。
    }
  }

  /**
   * 打开本应用的系统详情页。
   *
   * 厂商的自启动开关没有通用 Intent，位置也因厂商而异（vivo/小米/OPPO 在「权限」里，
   * 华为叫「应用启动管理」），所以只跳详情页并提示用户自己找这个开关。
   */
  async function openAppSettings() {
    try {
      await invoke("open_app_settings");
      notify("请在应用详情页找到「自启动」并开启", "info");
    } catch (error) {
      notify(nativeErrorMessage(error, "打开应用设置失败"), "error");
    }
  }

  /** 申请加入电池优化白名单；结果由系统弹窗决定，返回后再读一次状态。 */
  async function requestBatteryOptimization() {
    if (backgroundPending.value) return;
    backgroundPending.value = "battery";
    try {
      const granted = await invoke<boolean>("request_ignore_battery_optimizations");
      if (!granted) notify("未加入电池优化白名单，后台签到可能被系统中断", "info");
    } catch (error) {
      notify(nativeErrorMessage(error, "无法申请电池优化白名单"), "error");
    } finally {
      backgroundPending.value = null;
      await loadBackgroundPermissions();
    }
  }

  /**
   * 请求通知权限。
   *
   * Android 13 起弹系统授权框，更早的版本由原生侧打开系统通知页，两者都要等用户
   * 操作完才返回。用户拒绝只影响提醒、不阻断自动签到，所以这里不抛错。
   */
  async function requestNotificationPermission() {
    if (!isAndroid || backgroundPending.value) return false;
    backgroundPending.value = "notification";
    try {
      const granted = await invoke<boolean>("request_notification_permission");
      if (!granted) notify("未授予通知权限，签到结果不会提醒", "info");
      return granted;
    } catch (error) {
      notify(nativeErrorMessage(error, "无法打开通知设置"), "error");
      return false;
    } finally {
      backgroundPending.value = null;
      await loadBackgroundPermissions();
    }
  }

  async function updateContentDictionary(chapterId: number) {
    if (!Number.isInteger(chapterId) || chapterId <= 0) {
      notify("请输入有效的章节编号");
      return;
    }
    contentDictionaryChapterId.value = chapterId;
    contentDictionaryUpdating.value = true;
    try {
      const result = await invoke<{ size: number; added?: number }>(
        "update_content_dictionary",
        { chapterId },
      );
      contentDictionarySize.value = result.size;
      notify("正文恢复字典已更新，新增 " + (result.added ?? 0) + " 个字符");
    } catch (error) {
      notify(nativeErrorMessage(error, "正文恢复字典更新失败"), "error");
    } finally {
      contentDictionaryUpdating.value = false;
    }
  }

  async function saveSettings(value: AppSettings) {
    settingsSaving.value = true;
    try {
      const saved = await persist(value);
      if (
        saved.storage.useExternalStorage &&
        !(await invoke<boolean>("ensure_external_storage_access"))
      ) {
        notify("外部存储不可用，请授权或在设置中切换为内部存储", "error");
        return;
      }
      settingsOpen.value = false;
      notify("设置已更新");
    } catch (error) {
      notify(nativeErrorMessage(error, "保存设置失败"), "error");
    } finally {
      settingsSaving.value = false;
    }
  }

  /**
   * 只改自动签到开关。
   *
   * 不关闭设置弹窗、也不触发存储授权检查——这两个副作用只属于「用户提交整份
   * 设置」。签到域通过它写入，保证设置状态只有这一份。
   */
  async function setAutoSignin(enabled: boolean) {
    const current = await ensureSettings();
    return persist({
      ...current,
      signin: { ...current.signin, autoSigninEnabled: enabled },
    });
  }

  return {
    settingsOpen,
    settings,
    settingsSaving,
    contentDictionarySize,
    contentDictionaryUpdating,
    contentDictionaryChapterId,
    backgroundPermissions,
    backgroundPending,
    loadSettings,
    ensureSettings,
    loadBackgroundPermissions,
    openSettings,
    openAppSettings,
    requestBatteryOptimization,
    requestNotificationPermission,
    updateContentDictionary,
    saveSettings,
    setAutoSignin,
  };
}

/** 该域对外暴露的完整上下文；页面只应通过 `useSettings` 取用。 */
export type SettingsDesk = ReturnType<typeof createSettingsDesk>;
