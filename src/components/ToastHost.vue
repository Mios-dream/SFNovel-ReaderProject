<script setup lang="ts">
import {
  CheckCircle2,
  CircleAlert,
  Info,
  TriangleAlert,
  X,
} from "@lucide/vue";
import { useToast, type ToastType } from "../composables/useToast";

const { toasts, dismissToast } = useToast();
const icons = {
  info: Info,
  success: CheckCircle2,
  warning: TriangleAlert,
  error: CircleAlert,
} satisfies Record<ToastType, typeof Info>;
</script>

<template>
  <div class="toast-host" aria-live="polite" aria-atomic="false">
    <TransitionGroup name="toast" tag="div" class="toast-list">
      <article
        v-for="toast in toasts"
        :key="toast.id"
        class="toast"
        :class="`toast--${toast.type}`"
        :role="toast.type === 'error' ? 'alert' : 'status'"
      >
        <span class="toast-icon" aria-hidden="true">
          <component :is="icons[toast.type]" :size="18" :stroke-width="2.2" />
        </span>
        <p class="toast-message">{{ toast.message }}</p>
        <button
          class="toast-dismiss"
          type="button"
          aria-label="关闭提示"
          title="关闭提示"
          @click="dismissToast(toast.id)"
        >
          <X :size="15" />
        </button>
      </article>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-host {
  position: fixed;
  top: max(18px, env(safe-area-inset-top));
  right: max(18px, env(safe-area-inset-right));
  z-index: 1000;
  width: min(380px, calc(100vw - 36px));
  pointer-events: none;
}

.toast-list {
  display: grid;
  gap: 10px;
}

.toast {
  --toast-accent: var(--theme-color-dark);
  --toast-icon-background: #fff0e7;
  position: relative;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  min-width: 0;
  overflow: hidden;
  padding: 12px 11px 12px 14px;
  border: 1px solid rgba(226, 148, 100, 0.2);
  border-radius: 16px;
  color: var(--ink);
  background: rgba(255, 251, 247, 0.94);
  box-shadow: 0 14px 34px rgba(101, 65, 57, 0.19);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
  pointer-events: auto;
}

.toast::before {
  position: absolute;
  inset: 0 auto 0 0;
  width: 3px;
  content: "";
  background: var(--toast-accent);
}

.toast--success {
  --toast-accent: #41956a;
  --toast-icon-background: #e8f7ed;
}

.toast--warning {
  --toast-accent: #c48731;
  --toast-icon-background: #fff4da;
}

.toast--error {
  --toast-accent: #cf5b4f;
  --toast-icon-background: #ffebe7;
}

.toast-icon {
  display: inline-flex;
  width: 32px;
  height: 32px;
  align-items: center;
  justify-content: center;
  flex: 0 0 auto;
  border-radius: 10px;
  color: var(--toast-accent);
  background: var(--toast-icon-background);
}

.toast-message {
  min-width: 0;
  max-height: 4.35em;
  margin: 0;
  overflow: hidden;
  color: #5b4740;
  font-size: 13px;
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.toast-dismiss {
  display: inline-flex;
  width: 28px;
  height: 28px;
  align-items: center;
  justify-content: center;
  flex: 0 0 auto;
  padding: 0;
  border: 0;
  border-radius: 9px;
  color: #a88d83;
  background: transparent;
}

.toast-dismiss:hover,
.toast-dismiss:focus-visible {
  color: var(--toast-accent);
  background: var(--toast-icon-background);
  outline: none;
}

.toast-enter-active,
.toast-leave-active,
.toast-move {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(-8px) scale(0.98);
}

.toast-leave-active {
  position: absolute;
  right: 0;
  left: 0;
}

@media (max-width: 760px) {
  .toast-host {
    top: auto;
    right: auto;
    bottom: calc(100px + env(safe-area-inset-bottom));
    left: 50%;
    width: min(320px, calc(100vw - 32px));
    transform: translateX(-50%);
  }

  .toast-list {
    justify-items: center;
    gap: 8px;
  }

  .toast {
    width: fit-content;
    max-width: 100%;
    padding: 8px 9px 8px 10px;
    gap: 8px;
    border-color: rgba(226, 148, 100, 0.14);
    border-radius: 12px;
    background: rgba(255, 252, 249, 0.9);
    box-shadow: 0 8px 20px rgba(101, 65, 57, 0.11);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
  }

  .toast::before {
    width: 2px;
  }

  .toast-icon {
    width: 25px;
    height: 25px;
    border-radius: 8px;
  }

  .toast-icon :deep(svg) {
    width: 16px;
    height: 16px;
  }

  .toast-message {
    max-height: 2.8em;
    font-size: 12px;
    line-height: 1.4;
  }

  .toast-dismiss {
    width: 24px;
    height: 24px;
    border-radius: 7px;
  }

  .toast-leave-active {
    width: fit-content;
    max-width: 100%;
    margin: 0 auto;
  }
}
</style>
