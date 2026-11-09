<script setup lang="ts">
/**
 * 抽屉外壳：在 `ModalShell` 之上补齐抽屉骨架——手柄、标题栏、关闭按钮与滚动区。
 *
 * - 落点只有三种：`start` 左侧、`end` 右侧、`bottom` 贴底；窄屏下 `end` 与
 *   `bottom` 一样贴底并露出抓手
 * - 面板宽度、圆角、投影、间距、滚动与底部安全区都在这里定死，调用方只写内容
 * - 滚动容器由本组件持有：虚拟列表通过 `bodyRef` 拿它，滚动监听通过 `scroll` 事件回传
 * - 遮罩、返回栈、焦点限制、Esc 与滚动锁沿用 `ModalShell`，不再各页各写一套
 */
import { useId, type ComponentPublicInstance } from "vue";
import { X } from "@lucide/vue";
import ModalShell from "./ModalShell.vue";

const props = withDefaults(
  defineProps<{
    /** 打开状态由父级驱动。 */
    open: boolean;
    /** 抽屉落点。 */
    align?: "start" | "end" | "bottom";
    /** 侧边抽屉宽度：`list` 给目录列表，`detail` 给长文详情。贴底抽屉不受影响。 */
    size?: "list" | "detail";
    title: string;
    /** 标题上方的一行小字，通常是作品名。 */
    subtitle?: string;
    /** 需要自己接管滚动容器时传入（虚拟列表要把容器当窗口）。 */
    bodyRef?: (element: HTMLElement | null) => void;
  }>(),
  { align: "end", size: "list" },
);
const emit = defineEmits<{ close: []; scroll: [] }>();

const titleId = useId();

function setBody(element: Element | ComponentPublicInstance | null) {
  props.bodyRef?.(element instanceof HTMLElement ? element : null);
}
</script>

<template>
  <ModalShell :open="open" :align="align" @close="emit('close')">
    <section
      class="drawer"
      :class="[`drawer--${align}`, `drawer--${size}`]"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
    >
      <div v-if="align !== 'start'" class="drawer-handle" aria-hidden="true" />
      <header class="drawer-header">
        <div class="drawer-title">
          <small v-if="subtitle">{{ subtitle }}</small>
          <h2 :id="titleId">{{ title }}</h2>
        </div>
        <div class="drawer-actions">
          <slot name="actions" />
          <button
            class="drawer-close"
            type="button"
            title="关闭"
            aria-label="关闭"
            @click="emit('close')"
          >
            <X :size="18" />
          </button>
        </div>
      </header>
      <div
        :ref="setBody"
        class="drawer-body"
        @scroll.passive="emit('scroll')"
      >
        <slot />
      </div>
    </section>
  </ModalShell>
</template>

<style scoped>
.drawer {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid #ece1d2;
  background: #fffdf8;
  color: #483f39;
  box-shadow: 0 20px 48px rgba(46, 40, 33, 0.22);
}

/* 侧边抽屉贴边满高，圆角只留在朝向页面的一侧。 */
.drawer--start,
.drawer--end {
  width: min(360px, 84vw);
  height: 100%;
  border-top: 0;
  border-bottom: 0;
}
.drawer--start {
  border-left: 0;
  border-radius: 0 22px 22px 0;
}
.drawer--end {
  border-right: 0;
  border-radius: 22px 0 0 22px;
}
.drawer--start.drawer--detail,
.drawer--end.drawer--detail {
  width: min(520px, 100%);
}

/* 贴底抽屉：左右通栏，向上长高，圆角只留在顶部。 */
.drawer--bottom {
  width: 100%;
  max-height: min(82dvh, 720px);
  border-right: 0;
  border-bottom: 0;
  border-left: 0;
  border-radius: 24px 24px 0 0;
}

.drawer-handle {
  width: 40px;
  height: 4px;
  flex: 0 0 auto;
  margin: 8px auto 0;
  border-radius: 2px;
  background: #ddd0c0;
}
/* 右侧抽屉只有变成贴底时才有抓手。 */
.drawer--end .drawer-handle {
  display: none;
}

.drawer-header {
  display: flex;
  min-height: 58px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 0 14px 0 20px;
  border-bottom: 1px solid #efe7da;
}
/* 侧边抽屉顶到屏幕上沿，给状态栏留出安全区。 */
.drawer--start .drawer-header,
.drawer--end .drawer-header {
  padding-top: env(safe-area-inset-top);
}

.drawer-title {
  min-width: 0;
}
.drawer-title small {
  display: block;
  overflow: hidden;
  color: #b59885;
  font-size: 12px;
  line-height: 1.4;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.drawer-title h2 {
  margin: 2px 0 0;
  overflow: hidden;
  color: #443e36;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 18px;
  line-height: 1.3;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.drawer-actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 10px;
}
.drawer-close {
  display: inline-flex;
  width: 34px;
  height: 34px;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 50%;
  color: #79695e;
  background: #f4eee4;
  transition:
    color 150ms ease,
    background-color 150ms ease,
    transform 150ms ease;
}
.drawer-close:hover {
  color: #fff;
  background: var(--theme-color);
}
.drawer-close:active {
  transform: scale(0.94);
}

.drawer-body {
  min-height: 0;
  flex: 1 1 auto;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding-bottom: env(safe-area-inset-bottom);
}

@media (max-width: 900px) {
  .drawer--end,
  .drawer--end.drawer--detail {
    width: 100%;
    height: auto;
    max-height: min(82dvh, 720px);
    border: 1px solid #ece1d2;
    border-right: 0;
    border-bottom: 0;
    border-left: 0;
    border-radius: 24px 24px 0 0;
  }
  .drawer--end .drawer-handle {
    display: block;
  }
}
</style>
