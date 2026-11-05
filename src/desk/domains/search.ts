import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { Novel } from "../../types";
import { nativeErrorMessage } from "../../utils/native";
import type { Notify } from "../../composables/useToast";

type SearchDeskOptions = {
  refreshAuthStatus: () => Promise<void>;
  notify: Notify;
};

export function createSearchDesk({ refreshAuthStatus, notify }: SearchDeskOptions) {
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
      notify(nativeErrorMessage(error, "搜索失败"), "error");
    } finally {
      loading.value = false;
    }
  }

  function clearSearch() {
    query.value = "";
    results.value = [];
    loading.value = false;
    searched.value = false;
  }

  return { query, results, loading, searched, search, clearSearch };
}

/** 该域对外暴露的完整上下文；页面只应通过 `useSearch` 取用。 */
export type SearchDesk = ReturnType<typeof createSearchDesk>;
