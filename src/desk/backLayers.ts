import { onBeforeUnmount, watch } from "vue";

/**
 * 浮层返回栈。
 *
 * 主流平台（Android `OnBackPressedDispatcher`、Flutter `PopScope`、iOS sheet dismiss）
 * 的共同模型：每个「按返回时应当消失的层」在打开期间持有一个回调，调度器只执行
 * 最上层的一个，命中即停。应用里分两级——`back()` 先问 `dismissTopLayer()`，没有
 * 浮层才按路由返回表回退一层。
 *
 * 注册发生在「打开时」而非「挂载时」：顺序即打开顺序，也就是屏幕叠放顺序。若按
 * 模板里的挂载顺序注册，靠后声明的弹窗会永远压住先打开的那一层。
 */
type BackLayer = {
  /** 是否仍打开；退场动画期间必须为 false，避免返回键打到正在消失的层上。 */
  isOpen: () => boolean;
  /** 关闭该层，通常是把驱动 `open` 的 ref 置回 false。 */
  dismiss: () => void;
};

const layers: BackLayer[] = [];

/** 入栈，返回注销函数。调用方在层关闭或组件卸载时调用。 */
function registerBackLayer(layer: BackLayer) {
  layers.push(layer);
  return () => {
    const index = layers.indexOf(layer);
    if (index >= 0) layers.splice(index, 1);
  };
}

/**
 * 只关最上层浮层；没有可关的层时返回 false，交回给路由层处理。
 *
 * 层只要还开着就继续持有返回事件：保存中拒绝关闭的弹窗不会被第二次返回越过，
 * 异步关闭（例如讨论详情写 URL）也只会让一次快速连击落空，不会误跳页面。
 */
export function dismissTopLayer(): boolean {
  for (let index = layers.length - 1; index >= 0; index--) {
    const layer = layers[index];
    if (!layer.isOpen()) continue;
    layer.dismiss();
    return true;
  }
  return false;
}

/**
 * 在组件 setup 中把一个 `open` 状态接进返回栈。
 *
 * `sync` flush 保证关闭时立即注销：退场动画还在播，这一层就已经不再响应返回。
 * 每次打开都生成新的层对象，关闭与重开不会互相污染。
 */
export function useBackLayer(open: () => boolean, dismiss: () => void) {
  let unregister: (() => void) | undefined;

  watch(
    open,
    (isOpen) => {
      if (isOpen) {
        unregister ??= registerBackLayer({ isOpen: open, dismiss });
      } else {
        unregister?.();
        unregister = undefined;
      }
    },
    { immediate: true, flush: "sync" },
  );

  onBeforeUnmount(() => {
    unregister?.();
    unregister = undefined;
  });
}
