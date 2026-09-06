import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { RequestPolicy } from "../types";
import type { Notify } from "./deskShared";

export function useRequestPolicy(notify: Notify) {
  const requestPolicyOpen = ref(false);
  const requestPolicy = ref<RequestPolicy>({
    requestIntervalMs: 500,
    maxConcurrentDownloads: 1,
    appFallbackEnabled: true,
    androidDeviceReportEnabled: true,
  });
  const requestPolicySaving = ref(false);
  const contentDictionarySize = ref(0);
  const contentDictionaryUpdating = ref(false);
  const contentDictionaryChapterId = ref(8436696);

  async function openRequestPolicy() {
    requestPolicyOpen.value = true;
    try {
      requestPolicy.value = await invoke<RequestPolicy>("get_request_policy");
      const dictionary = await invoke<{ size: number }>("get_content_dictionary");
      contentDictionarySize.value = dictionary.size;
    } catch (error) {
      notify(error instanceof Error ? error.message : "读取请求设置失败");
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
      notify(`正文恢复字典已更新，新增 ${result.added ?? 0} 个字符`);
    } catch (error) {
      notify(error instanceof Error ? error.message : "正文恢复字典更新失败");
    } finally {
      contentDictionaryUpdating.value = false;
    }
  }

  async function saveRequestPolicy(policy: RequestPolicy) {
    requestPolicySaving.value = true;
    try {
      requestPolicy.value = await invoke<RequestPolicy>("save_request_policy", {
        policy,
      });
      requestPolicyOpen.value = false;
      notify("请求设置已更新");
    } catch (error) {
      notify(error instanceof Error ? error.message : "保存请求设置失败");
    } finally {
      requestPolicySaving.value = false;
    }
  }

  return {
    requestPolicyOpen,
    requestPolicy,
    requestPolicySaving,
    contentDictionarySize,
    contentDictionaryUpdating,
    contentDictionaryChapterId,
    openRequestPolicy,
    updateContentDictionary,
    saveRequestPolicy,
  };
}
