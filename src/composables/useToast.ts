import { ref } from "vue";

export type ToastType = "info" | "success" | "warning" | "error";

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

export function errorMessage(error: unknown, fallback: string) {
  if (typeof error === "string" && error.trim()) return error.trim();
  if (error instanceof Error && error.message.trim()) return error.message.trim();
  if (error && typeof error === "object") {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) return message.trim();
  }
  return fallback;
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
  const message = errorMessage(error, fallback);
  console.error(`[SF Novel Flow] ${fallback}`, error);
  return showToast(message, "error");
}

export function useToast() {
  return { toasts, showToast, dismissToast };
}
