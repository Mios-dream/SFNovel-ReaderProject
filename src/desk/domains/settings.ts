import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { AppSettings } from "../../types";
import type { Notify } from "../../composables/useToast";
import { nativeErrorMessage } from "../../utils/native";

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
      await loadSettings();
      const dictionary = await invoke<{ size: number }>("get_content_dictionary");
      contentDictionarySize.value = dictionary.size;
    } catch (error) {
      notify(nativeErrorMessage(error, "读取设置失败"), "error");
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
    loadSettings,
    ensureSettings,
    openSettings,
    updateContentDictionary,
    saveSettings,
    setAutoSignin,
  };
}

/** 该域对外暴露的完整上下文；页面只应通过 `useSettings` 取用。 */
export type SettingsDesk = ReturnType<typeof createSettingsDesk>;
