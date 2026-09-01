import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { Novel } from "../types";
import { nativeErrorMessage, type Notify } from "./deskShared";

type UseNovelSearchOptions = {
  refreshAuthStatus: () => Promise<void>;
  notify: Notify;
};

export function useNovelSearch({ refreshAuthStatus, notify }: UseNovelSearchOptions) {
  const query = ref("");
  const results = ref<Novel[]>([]);
  const loading = ref(false);
  const searched = ref(false);

  async function search() {
    if (!query.value.trim()) return;
    await refreshAuthStatus();
    loading.value = true;
    searched.value = true;
    try {
      results.value = await invoke<Novel[]>("search_novels", {
        query: query.value.trim(),
      });
    } catch (error) {
      notify(nativeErrorMessage(error, "搜索失败"));
    } finally {
      loading.value = false;
    }
  }

  return { query, results, loading, searched, search };
}
