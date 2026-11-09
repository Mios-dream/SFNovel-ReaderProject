<script lang="ts">
/**
 * 弹窗外壳：遮罩、层级、进出场动画与全部交互都在这里定义，使用方只提供内容。
 *
 * - 遮罩颜色、模糊、内边距、z-index 全站统一，不接受外部传入
 * - 打开期间锁住页面滚动，关闭后原地恢复（按层计数，嵌套弹窗不会提前解锁）
 * - 滚轮与触摸在层内没有可滚动容器时不再落到背后页面；层内滚动到边界靠
 *   `[data-modal-layer] * { overscroll-behavior: contain }` 兜住惯性
 * - Esc 只关最上层，Tab 焦点收在层内，关闭后焦点还给触发元素
 * - 只有按下与抬手都在遮罩上、且位移不超过 8px 才算点击遮罩
 *
 * 由 `open` 驱动挂载，进出场动画在组件内部完成，调用方不要再套 `<Transition>`。
 */

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** 已打开的层，顺序即叠放顺序，末位是最上层。 */
const layers: HTMLElement[] = [];

function syncPageLock() {
  document.documentElement.classList.toggle("modal-open", layers.length > 0);
}

function isScrollable(element: HTMLElement) {
  if (
    element.scrollHeight <= element.clientHeight + 1 &&
    element.scrollWidth <= element.clientWidth + 1
  ) {
    return false;
  }
  const style = getComputedStyle(element);
  return (
    (/(auto|scroll)/.test(style.overflowY) &&
      element.scrollHeight > element.clientHeight + 1) ||
    (/(auto|scroll)/.test(style.overflowX) &&
      element.scrollWidth > element.clientWidth + 1)
  );
}

/** 事件目标到遮罩之间是否存在能自行消费滚动的容器。 */
function consumesScroll(target: EventTarget | null, root: HTMLElement) {
  if (!(target instanceof Element) || !root.contains(target)) return false;
  let node: Element | null = target;
  while (node) {
    if (node instanceof HTMLElement && isScrollable(node)) return true;
    if (node === root) break;
    node = node.parentElement;
  }
  return false;
}
</script>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    /** 打开状态由父级驱动，关闭时组件内部播放完退场动画再卸载。 */
    open: boolean;
    /**
     * 面板落点。只表达位置差异，样式与动画仍由组件统一提供：
     * `sheet` 桌面居中、窄屏贴底；`start` 左侧抽屉；`end` 右侧抽屉（窄屏贴底）；
     * `bottom` 贴底抽屉。
     */
    align?: "center" | "sheet" | "start" | "end" | "bottom";
  }>(),
  { align: "center" },
);
const emit = defineEmits<{ close: [] }>();

const root = ref<HTMLElement | null>(null);
/** 当前已挂到层栈上的元素，退场阶段仍需能摘掉监听。 */
let attached: HTMLElement | null = null;
let restoreFocus: HTMLElement | null = null;
let downOnBackdrop = false;
let downX = 0;
let downY = 0;

function onWheel(event: WheelEvent) {
  if (!root.value) return;
  if (!consumesScroll(event.target, root.value)) event.preventDefault();
}

function onTouchMove(event: TouchEvent) {
  if (!root.value) return;
  if (!consumesScroll(event.target, root.value)) event.preventDefault();
}

function onPointerDown(event: PointerEvent) {
  downOnBackdrop = event.target === root.value;
  downX = event.clientX;
  downY = event.clientY;
}

function onClick(event: MouseEvent) {
  if (!downOnBackdrop || event.target !== root.value) return;
  if (Math.hypot(event.clientX - downX, event.clientY - downY) > 8) return;
  emit("close");
}

function onKeydown(event: KeyboardEvent) {
  const element = root.value;
  if (!element || layers[layers.length - 1] !== element) return;
  if (event.key === "Escape") {
    event.preventDefault();
    emit("close");
    return;
  }
  if (event.key !== "Tab") return;
  const items = Array.from(
    element.querySelectorAll<HTMLElement>(FOCUSABLE),
  ).filter((item) => item.getClientRects().length > 0);
  if (!items.length) {
    event.preventDefault();
    return;
  }
  const active = document.activeElement;
  const inside = active instanceof HTMLElement && element.contains(active);
  if (event.shiftKey) {
    if (!inside || active === items[0] || active === element) {
      event.preventDefault();
      items[items.length - 1].focus();
    }
    return;
  }
  if (!inside || active === items[items.length - 1]) {
    event.preventDefault();
    items[0].focus();
  }
}

function attach(element: HTMLElement) {
  if (attached === element) return;
  detach();
  attached = element;
  restoreFocus =
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
  layers.push(element);
  syncPageLock();
  element.addEventListener("wheel", onWheel, { passive: false });
  element.addEventListener("touchmove", onTouchMove, { passive: false });
  document.addEventListener("keydown", onKeydown, true);
  element.focus({ preventScroll: true });
}

function detach() {
  const element = attached;
  if (!element) return;
  attached = null;
  document.removeEventListener("keydown", onKeydown, true);
  element.removeEventListener("wheel", onWheel);
  element.removeEventListener("touchmove", onTouchMove);
  const index = layers.indexOf(element);
  if (index >= 0) layers.splice(index, 1);
  syncPageLock();
  if (restoreFocus?.isConnected) restoreFocus.focus({ preventScroll: true });
  restoreFocus = null;
}

/** 退场动画结束后才真正解除滚动锁与焦点限制。 */
function onAfterLeave() {
  if (!props.open) detach();
}

watch(
  () => props.open,
  async (open) => {
    if (!open) return;
    await nextTick();
    if (root.value) attach(root.value);
  },
  { immediate: true, flush: "post" },
);

onBeforeUnmount(detach);
</script>

<template>
  <Transition name="modal-shell" appear @after-leave="onAfterLeave">
    <div
      v-if="open"
      ref="root"
      class="modal-shell"
      :class="`modal-shell--${align}`"
      data-modal-layer
      tabindex="-1"
      @pointerdown="onPointerDown"
      @click="onClick"
    >
      <slot />
    </div>
  </Transition>
</template>

<style scoped>
/* 全站统一的遮罩：暖黑色 + 轻微背景虚化，与 glass 面板的暖色调一致。 */
.modal-shell {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 18px;
  background: rgba(34, 25, 20, 0.34);
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
  outline: none;
}
.modal-shell--start {
  justify-content: flex-start;
  padding: 0;
}
.modal-shell--end {
  justify-content: flex-end;
  padding: 0;
}
.modal-shell--bottom {
  align-items: flex-end;
  padding: 0;
}
@media (max-width: 680px) {
  .modal-shell {
    padding: 10px;
  }
}
/* 设置、更新这类长内容在窄屏改成贴底部弹出。 */
@media (max-width: 600px) {
  .modal-shell--sheet {
    align-items: flex-end;
    padding: 0;
  }
}
/* 右侧面板在窄屏没有侧边空间，改成贴底部。 */
@media (max-width: 900px) {
  .modal-shell--end {
    align-items: flex-end;
  }
}

/* 遮罩淡入淡出，时长与面板一致，避免退场时被提前截断。 */
.modal-shell-enter-active {
  animation: modal-scrim-in 200ms ease-out;
}
.modal-shell-leave-active {
  animation: modal-scrim-out 160ms ease-in;
}
.modal-shell-enter-active :deep(> *) {
  animation: modal-panel-in 200ms cubic-bezier(0.22, 1, 0.36, 1);
}
.modal-shell-leave-active :deep(> *) {
  animation: modal-panel-out 160ms ease-in;
}
/* 入场方向由落点决定，其余表现一致。 */
.modal-shell--start.modal-shell-enter-active :deep(> *) {
  animation-name: modal-panel-in-start;
}
.modal-shell--end.modal-shell-enter-active :deep(> *) {
  animation-name: modal-panel-in-end;
}
.modal-shell--bottom.modal-shell-enter-active :deep(> *),
.modal-shell--sheet.modal-shell-enter-active :deep(> *) {
  animation-name: modal-panel-in-bottom;
}
@media (max-width: 900px) {
  .modal-shell--end.modal-shell-enter-active :deep(> *) {
    animation-name: modal-panel-in-bottom;
  }
}

@keyframes modal-scrim-in {
  from {
    opacity: 0;
  }
}
@keyframes modal-scrim-out {
  to {
    opacity: 0;
  }
}
@keyframes modal-panel-in {
  from {
    opacity: 0;
    transform: scale(0.96);
  }
}
@keyframes modal-panel-out {
  to {
    opacity: 0;
    transform: scale(0.99);
  }
}
@keyframes modal-panel-in-start {
  from {
    transform: translateX(-100%);
  }
}
@keyframes modal-panel-in-end {
  from {
    opacity: 0;
    transform: translateX(24px);
  }
}
@keyframes modal-panel-in-bottom {
  from {
    opacity: 0;
    transform: translateY(18px);
  }
}

@media (prefers-reduced-motion: reduce) {
  .modal-shell-enter-active,
  .modal-shell-leave-active,
  .modal-shell-enter-active :deep(> *),
  .modal-shell-leave-active :deep(> *) {
    animation: none;
  }
}
</style>
