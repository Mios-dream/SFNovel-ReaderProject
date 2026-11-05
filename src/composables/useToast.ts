import { ref } from "vue";
import { messageOf } from "../utils/native";

export type ToastType = "info" | "success" | "warning" | "error";

/** 域注入的提示函数；默认实现是 `showToast`，测试或预览可替换。 */
export type Notify = (message: string, type?: ToastType) => void;

export type ToastItem = {
  id: number;
  message: string;
  type: ToastType;
};

const toasts = ref<ToastItem[]>([]);
const timers = new Map<number, number>();
let nextToastId = 0;

function clearToastTimer(id: number) {
  const timer = timers.get(id);
  if (timer === undefined) return;
  window.clearTimeout(timer);
  timers.delete(id);
}

export function dismissToast(id: number) {
  clearToastTimer(id);
  toasts.value = toasts.value.filter((toast) => toast.id !== id);
}

export function showToast(
  message: string,
  type: ToastType = "info",
  duration = type === "error" ? 5600 : 3200,
) {
  const text = message.trim();
  if (!text) return;

  const duplicate = toasts.value.find(
    (toast) => toast.message === text && toast.type === type,
  );
  if (duplicate) return duplicate.id;

  const id = ++nextToastId;
  toasts.value = [
    ...toasts.value.slice(-3),
    { id, message: text, type },
  ];
  timers.set(id, window.setTimeout(() => dismissToast(id), duration));
  return id;
}

export function reportError(error: unknown, fallback: string) {
  const message = messageOf(error, fallback);
  console.error(`[SF Novel Flow] ${fallback}`, error);
  return showToast(message, "error");
}

export function useToast() {
  return { toasts, showToast, dismissToast };
}
