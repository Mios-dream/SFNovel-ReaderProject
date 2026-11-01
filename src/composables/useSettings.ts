import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { AppSettings } from "../types";
import type { Notify } from "./deskShared";

export function useSettings(notify: Notify) {
  const settingsOpen = ref(false);
  const settings = ref<AppSettings>({
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
    },
  });
  const settingsSaving = ref(false);
  const contentDictionarySize = ref(0);
  const contentDictionaryUpdating = ref(false);
  const contentDictionaryChapterId = ref(8436696);

  async function openSettings() {
    settingsOpen.value = true;
    try {
      settings.value = await invoke<AppSettings>("get_settings");
      const dictionary = await invoke<{ size: number }>("get_content_dictionary");
      contentDictionarySize.value = dictionary.size;
    } catch (error) {
      notify(error instanceof Error ? error.message : "读取设置失败", "error");
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
      notify(error instanceof Error ? error.message : "正文恢复字典更新失败", "error");
    } finally {
      contentDictionaryUpdating.value = false;
    }
  }

  async function saveSettings(value: AppSettings) {
    settingsSaving.value = true;
    try {
      const saved = await invoke<AppSettings>("save_settings", {
        settings: value,
      });
      settings.value = saved;
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
      notify(error instanceof Error ? error.message : "保存设置失败", "error");
    } finally {
      settingsSaving.value = false;
    }
  }

  return {
    settingsOpen,
    settings,
    settingsSaving,
    contentDictionarySize,
    contentDictionaryUpdating,
    contentDictionaryChapterId,
    openSettings,
    updateContentDictionary,
    saveSettings,
  };
}
