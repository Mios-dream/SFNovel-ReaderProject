<script lang="ts">
/** 底部控制栏的一个入口：图标插槽名、文案、当前是否展开对应面板。 */
export type ReaderAction = {
  key: string;
  label: string;
  active?: boolean;
};
</script>

<script setup lang="ts">
/**
 * 阅读器外壳：顶部浮动条 + 底部工具坞，小说与漫画共用一套。
 *
 * - 顶栏只有返回与标题；目录、设置这类入口全部收在底部控制栏，两个阅读器位置一致
 * - 控制栏入口由 `actions` 决定（键、文案、选中态），图标用同名插槽传入，
 *   按钮本身由本组件渲染，宽度与间距不会因为各页自己排列而漂移
 * - 面板区是 `panel` 插槽：内容归页面，容器与设置行、分段按钮、进度条等基元在这里
 * - 面板与插槽内容挂在 `footer` 之下，因此这些基元用 `:deep()` 命中
 */
import { ChevronLeft } from "@lucide/vue";

defineProps<{
  visible: boolean;
  title: string;
  subtitle?: string;
  actions: ReaderAction[];
}>();
const emit = defineEmits<{ back: []; action: [key: string] }>();
</script>

<template>
  <Transition name="reader-top">
    <header
      v-show="visible"
      class="reader-floating-bar reader-top-bar"
      @click.stop
    >
      <button
        class="reader-icon-button"
        type="button"
        title="返回书籍详情"
        aria-label="返回书籍详情"
        @click="emit('back')"
      >
        <ChevronLeft :size="21" />
      </button>
      <div class="reader-heading">
        <small v-if="subtitle">{{ subtitle }}</small>
        <strong>{{ title }}</strong>
      </div>
    </header>
  </Transition>

  <Transition name="reader-dock">
    <footer v-show="visible" class="reader-dock" @click.stop>
      <slot name="panel" />
      <nav class="reader-control-bar" aria-label="阅读设置">
        <button
          v-for="action in actions"
          :key="action.key"
          type="button"
          class="reader-control-action"
          :class="{ active: action.active }"
          :title="action.label"
          :aria-label="action.label"
          :aria-expanded="action.active === undefined ? undefined : action.active"
          @click="emit('action', action.key)"
        >
          <slot :name="action.key" />
          <span>{{ action.label }}</span>
        </button>
      </nav>
    </footer>
  </Transition>
</template>

<style scoped>
.reader-floating-bar,
.reader-dock {
  --reader-dock-width: min(430px, calc(100vw - 24px));
}

.reader-floating-bar {
  position: fixed;
  z-index: 30;
  display: flex;
  align-items: center;
  min-height: 52px;
}
.reader-top-bar {
  top: max(10px, env(safe-area-inset-top));
  left: 50%;
  width: var(--reader-dock-width);
  justify-content: flex-start;
  gap: 4px;
  padding: 5px 6px;
  border: 1px solid rgba(255, 255, 255, 0.74);
  border-radius: 999px;
  background: rgba(255, 252, 249, 0.9);
  box-shadow:
    0 18px 42px rgba(89, 57, 48, 0.18),
    0 2px 8px rgba(89, 57, 48, 0.06);
  transform: translateX(-50%);
  backdrop-filter: blur(22px) saturate(1.15);
  -webkit-backdrop-filter: blur(22px) saturate(1.15);
}
.reader-heading {
  display: flex;
  min-width: 0;
  flex: 1 1 auto;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 0 40px 0 0;
  color: var(--theme-color-dark);
}
.reader-heading small {
  flex: 0 0 auto;
  font-size: 14px;
}
.reader-heading strong {
  overflow: hidden;
  font-size: 14px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.reader-icon-button {
  display: inline-flex;
  width: 40px;
  height: 40px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 18px;
  color: var(--theme-color-dark);
  background: transparent;
}
.reader-icon-button:hover {
  background: var(--theme-color-light);
}

.reader-dock {
  position: fixed;
  z-index: 30;
  left: 50%;
  bottom: max(10px, env(safe-area-inset-bottom));
  display: flex;
  width: var(--reader-dock-width);
  flex-direction: column;
  align-items: center;
  gap: 8px;
  transform: translateX(-50%);
}
.reader-control-bar {
  display: grid;
  width: 100%;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 4px;
  border: 1px solid rgba(255, 255, 255, 0.74);
  border-radius: 999px;
  background: rgba(255, 250, 247, 0.9);
  box-shadow:
    0 18px 42px rgba(89, 57, 48, 0.18),
    0 2px 8px rgba(89, 57, 48, 0.06);
  backdrop-filter: blur(22px) saturate(1.15);
  -webkit-backdrop-filter: blur(22px) saturate(1.15);
}
.reader-control-action {
  display: flex;
  height: 54px;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 3px;
  border: 0;
  border-radius: 999px;
  color: var(--theme-color-dark);
  background: transparent;
  font-size: 12px;
  line-height: 1;
  transition:
    color 150ms ease,
    background 150ms ease,
    transform 150ms ease;
}
.reader-control-action:hover {
  background: rgba(242, 219, 205, 0.6);
}
.reader-control-action.active {
  color: #a85a39;
  background: rgba(226, 148, 100, 0.16);
}
.reader-control-action:active {
  transform: scale(0.96);
}

/* 面板基元：内容由页面写进 `panel` 插槽，样式留在这里保证两个阅读器一致。 */
.reader-dock :deep(.reader-panel) {
  width: min(340px, 100%);
  padding: 6px 10px;
  border: 1px solid rgba(255, 255, 255, 0.74);
  border-radius: 18px;
  background: rgba(255, 252, 249, 0.92);
  box-shadow: 0 12px 30px rgba(89, 57, 48, 0.13);
  backdrop-filter: blur(20px) saturate(1.1);
  -webkit-backdrop-filter: blur(20px) saturate(1.1);
}
.reader-dock :deep(.reader-setting-row) {
  display: flex;
  min-height: 34px;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
}
.reader-dock :deep(.reader-setting-row + .reader-setting-row) {
  margin-top: 2px;
  padding-top: 2px;
  border-top: 1px solid rgba(202, 174, 159, 0.24);
}
.reader-dock :deep(.reader-setting-label) {
  color: var(--muted);
  font-size: 12px;
}
.reader-dock :deep(.reader-segment) {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 2px;
  border-radius: 10px;
  background: rgba(242, 228, 218, 0.82);
}
.reader-dock :deep(.reader-segment button) {
  height: 28px;
  padding: 0 10px;
  border: 0;
  border-radius: 8px;
  color: #8b7167;
  background: transparent;
  font-size: 12px;
  transition:
    color 150ms ease,
    background 150ms ease;
}
.reader-dock :deep(.reader-segment button.active) {
  color: var(--theme-color-dark);
  background: #fffaf6;
  box-shadow: 0 1px 4px rgba(89, 57, 48, 0.1);
}
.reader-dock :deep(.reader-segment button:disabled) {
  opacity: 0.32;
}
/* 数值步进器：符号/图标按钮夹一个当前值，字号与缩放共用同一尺寸。 */
.reader-dock :deep(.reader-stepper) {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 2px;
  border-radius: 10px;
  background: rgba(242, 228, 218, 0.82);
}
.reader-dock :deep(.reader-stepper button) {
  display: inline-flex;
  width: 28px;
  height: 28px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 8px;
  color: var(--theme-color-dark);
  background: transparent;
  font-size: 16px;
  line-height: 1;
  transition:
    color 150ms ease,
    background 150ms ease,
    transform 150ms ease;
}
.reader-dock :deep(.reader-stepper button:hover:not(:disabled)) {
  background: #fffaf6;
}
.reader-dock :deep(.reader-stepper button:active:not(:disabled)) {
  transform: scale(0.92);
}
.reader-dock :deep(.reader-stepper button:disabled) {
  opacity: 0.3;
  cursor: default;
}
.reader-dock :deep(.reader-stepper-value) {
  min-width: 40px;
  color: var(--theme-color-dark);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  text-align: center;
}
/* 重置类动作与加减之间留一道竖线，避免连点时误触。 */
.reader-dock :deep(.reader-stepper-divider) {
  width: 1px;
  height: 15px;
  margin: 0 2px;
  background: rgba(202, 174, 159, 0.5);
}
.reader-dock :deep(.reader-mode-options) {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
}
.reader-dock :deep(.reader-mode-options button) {
  display: flex;
  height: 38px;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 0;
  border: 0;
  border-radius: 12px;
  color: #8b7167;
  background: rgba(242, 228, 218, 0.6);
  font-size: 12px;
  font-weight: 500;
  transition:
    color 150ms ease,
    background 150ms ease,
    box-shadow 150ms ease;
}
.reader-dock :deep(.reader-mode-options button.active) {
  color: var(--theme-color-dark);
  background: #fffaf6;
  box-shadow: 0 1px 5px rgba(89, 57, 48, 0.11);
}
.reader-dock :deep(.reader-progress-control) {
  display: flex;
  width: min(340px, calc(100% - 42px));
  min-height: 42px;
  align-items: center;
  gap: 6px;
  padding: 4px 5px;
  border: 1px solid rgba(255, 255, 255, 0.74);
  border-radius: 999px;
  background: rgba(255, 252, 249, 0.9);
  box-shadow: 0 10px 26px rgba(89, 57, 48, 0.12);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
}
.reader-dock :deep(.reader-turn-button) {
  display: inline-flex;
  width: 32px;
  height: 32px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 50%;
  color: var(--theme-color-dark);
  background: transparent;
  transition:
    color 150ms ease,
    background 150ms ease,
    transform 150ms ease;
}
.reader-dock :deep(.reader-turn-button:hover:not(:disabled)) {
  background: rgba(242, 219, 205, 0.7);
}
.reader-dock :deep(.reader-turn-button:active:not(:disabled)) {
  transform: scale(0.92);
}
.reader-dock :deep(.reader-turn-button:disabled) {
  opacity: 0.28;
  cursor: default;
}
.reader-dock :deep(.reader-progress-main) {
  display: flex;
  min-width: 0;
  flex: 1 1 auto;
  align-items: center;
}
.reader-dock :deep(.reader-progress-track) {
  height: 4px;
  flex: 1 1 auto;
  overflow: hidden;
  border-radius: 999px;
  background: rgba(201, 111, 72, 0.16);
}
.reader-dock :deep(.reader-progress-track span) {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #e29464, #cf7952);
  transition: width 180ms ease-out;
}
.reader-control-action:focus-visible,
.reader-dock :deep(.reader-turn-button:focus-visible),
  .reader-dock :deep(.reader-segment button:focus-visible),
  .reader-dock :deep(.reader-stepper button:focus-visible),
  .reader-dock :deep(.reader-mode-options button:focus-visible) {
  outline: 2px solid rgba(201, 111, 72, 0.55);
  outline-offset: 2px;
}

/* 进出场：顶栏自上方、坞自下方，面板在坞内单独淡入。 */
.reader-dock-enter-active {
  transition:
    opacity 240ms ease-out,
    transform 250ms cubic-bezier(0.2, 0.8, 0.2, 1);
}
.reader-dock-leave-active {
  transition:
    opacity 180ms ease-in,
    transform 200ms ease-in;
}
.reader-dock-enter-from,
.reader-dock-leave-to {
  opacity: 0;
  transform: translate(-50%, 16px) scale(0.97);
}
.reader-top-enter-active {
  transition:
    opacity 240ms ease-out,
    transform 250ms cubic-bezier(0.2, 0.8, 0.2, 1);
}
.reader-top-leave-active {
  transition:
    opacity 180ms ease-in,
    transform 200ms ease-in;
}
.reader-top-enter-from,
.reader-top-leave-to {
  opacity: 0;
  transform: translate(-50%, -14px) scale(0.97);
}
.reader-dock :deep(.reader-swap-enter-active) {
  transition:
    opacity 220ms ease-out,
    transform 240ms cubic-bezier(0.2, 0.8, 0.2, 1);
}
.reader-dock :deep(.reader-swap-leave-active) {
  transition:
    opacity 140ms ease-in,
    transform 160ms ease-in;
}
.reader-dock :deep(.reader-swap-enter-from),
.reader-dock :deep(.reader-swap-leave-to) {
  opacity: 0;
  transform: translateY(8px) scale(0.97);
}

@media (max-width: 760px) {
  .reader-floating-bar,
  .reader-dock {
    --reader-dock-width: min(330px, calc(100vw - 40px));
  }
  .reader-top-bar {
    top: max(8px, env(safe-area-inset-top));
    min-height: 44px;
    gap: 2px;
    padding: 4px 5px;
  }
  .reader-icon-button {
    width: 34px;
    height: 34px;
    border-radius: 15px;
  }
  .reader-heading {
    gap: 6px;
    padding-right: 34px;
  }
  .reader-heading small,
  .reader-heading strong {
    font-size: 13px;
  }
  .reader-dock {
    bottom: max(8px, env(safe-area-inset-bottom));
    gap: 6px;
  }
  .reader-control-action {
    height: 46px;
    gap: 2px;
  }
  .reader-control-action svg {
    width: 18px;
    height: 18px;
  }
  .reader-dock :deep(.reader-progress-control) {
    width: min(270px, calc(100% - 48px));
    min-height: 36px;
    gap: 5px;
    padding: 3px 4px;
  }
  .reader-dock :deep(.reader-turn-button) {
    width: 28px;
    height: 28px;
  }
  .reader-dock :deep(.reader-turn-button svg) {
    width: 16px;
    height: 16px;
  }
  .reader-dock :deep(.reader-progress-track) {
    height: 3px;
  }
  .reader-dock :deep(.reader-panel) {
    width: min(270px, calc(100% - 48px));
    padding: 4px 8px;
    border-radius: 16px;
  }
  .reader-dock :deep(.reader-setting-row) {
    min-height: 30px;
    gap: 12px;
  }
  .reader-dock :deep(.reader-segment button) {
    height: 26px;
    padding: 0 8px;
    border-radius: 7px;
  }
  .reader-dock :deep(.reader-stepper button) {
    width: 26px;
    height: 26px;
    border-radius: 7px;
  }
  .reader-dock :deep(.reader-stepper-value) {
    min-width: 36px;
    font-size: 12px;
  }
  .reader-dock :deep(.reader-mode-options button) {
    height: 32px;
    border-radius: 10px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .reader-dock-enter-active,
  .reader-dock-leave-active,
  .reader-top-enter-active,
  .reader-top-leave-active {
    transition: none;
  }
  .reader-control-action:active {
    transform: none;
  }
  .reader-dock :deep(.reader-stepper button:active) {
    transform: none;
  }
  .reader-dock :deep(.reader-swap-enter-active),
  .reader-dock :deep(.reader-swap-leave-active),
  .reader-dock :deep(.reader-progress-track span) {
    transition: none;
  }
}
</style>
