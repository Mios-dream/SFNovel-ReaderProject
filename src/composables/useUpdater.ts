import { invoke, isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ref } from "vue";
import { nativeErrorMessage, type Notify } from "./deskShared";

export type UpdateStatus = "idle" | "checking" | "available" | "error";

export type UpdateInfo = {
  currentVersion: string;
  version: string;
  notes?: string;
  releaseUrl: string;
};

const RELEASES_URL =
  "https://github.com/Mios-dream/SFNovel-ReaderProject/releases";

export function useUpdater(notify: Notify) {
  const currentVersion = ref("未知");
  const status = ref<UpdateStatus>("idle");
  const info = ref<UpdateInfo>();
  const error = ref("");
  const modalOpen = ref(false);

  async function loadCurrentVersion() {
    if (!isTauri()) return;
    try {
      currentVersion.value = await invoke<string>("app_runtime_version");
    } catch {
      currentVersion.value = "未知";
    }
  }

  async function checkForUpdate(manual = true) {
    if (!isTauri()) {
      if (manual) await openReleases();
      return;
    }
    if (status.value === "checking") return;
    error.value = "";
    info.value = undefined;
    status.value = "checking";
    if (manual) modalOpen.value = true;
    try {
      const found = await invoke<UpdateInfo | null>("check_update");
      info.value = found ?? undefined;
      if (found) {
        status.value = "available";
        modalOpen.value = true;
      } else {
        status.value = "idle";
        if (manual) {
          modalOpen.value = false;
          notify("当前已是最新版本");
        }
      }
    } catch (cause) {
      nativeErrorMessage(cause, "检查更新失败");
      status.value = "error";
      error.value = "无法获取更新信息，请前往 GitHub 查看最新版本";
      if (manual) modalOpen.value = true;
    }
  }

  async function openReleases() {
    try {
      await openUrl(info.value?.releaseUrl ?? RELEASES_URL);
    } catch (cause) {
      notify(nativeErrorMessage(cause, "无法打开 GitHub"), "error");
    }
  }

  function showUpdate() {
    modalOpen.value = true;
  }

  function closeModal() {
    modalOpen.value = false;
  }

  return {
    currentVersion,
    status,
    info,
    error,
    modalOpen,
    loadCurrentVersion,
    checkForUpdate,
    openReleases,
    showUpdate,
    closeModal,
  };
}
