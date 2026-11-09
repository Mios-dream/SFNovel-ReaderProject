<script setup lang="ts">
import { Check, ChevronDown } from "@lucide/vue";
import { computed, onBeforeUnmount, onMounted, ref, useId } from "vue";

type Option = {
  label: string;
  value: boolean;
};

const props = defineProps<{
  modelValue: boolean;
  options: Option[];
  label: string;
}>();
const emit = defineEmits<{
  "update:modelValue": [value: boolean];
}>();

const root = ref<HTMLElement>();
const trigger = ref<HTMLButtonElement>();
const menu = ref<HTMLElement>();
const open = ref(false);
const menuId = useId();
const selectedLabel = computed(
  () =>
    props.options.find((option) => option.value === props.modelValue)?.label ??
    "",
);

function openMenu() {
  if (open.value) return;
  open.value = true;
  requestAnimationFrame(() => {
    const selectedIndex = props.options.findIndex(
      (option) => option.value === props.modelValue,
    );
    optionButtons()[selectedIndex < 0 ? 0 : selectedIndex]?.focus();
  });
}

function closeMenu(restoreFocus = false) {
  open.value = false;
  if (restoreFocus) trigger.value?.focus();
}

function optionButtons() {
  return Array.from(
    menu.value?.querySelectorAll<HTMLButtonElement>("[role='option']") ?? [],
  );
}

function selectOption(value: boolean) {
  emit("update:modelValue", value);
  closeMenu(true);
}

function handleKeydown(event: KeyboardEvent) {
  const options = optionButtons();
  const index = options.indexOf(document.activeElement as HTMLButtonElement);

  if (event.key === "Escape" && open.value) {
    event.preventDefault();
    // 这是纯下拉菜单，不是返回层；别让 Esc 冒泡去关掉承载它的弹窗。
    event.stopPropagation();
    closeMenu(true);
    return;
  }

  if (!open.value) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      openMenu();
    }
    return;
  }

  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    const offset = event.key === "ArrowDown" ? 1 : -1;
    const nextIndex = (index + offset + options.length) % options.length;
    options[nextIndex]?.focus();
  }
}

function handlePointerDown(event: PointerEvent) {
  if (root.value && !root.value.contains(event.target as Node)) {
    closeMenu();
  }
}

function handleFocusOut(event: FocusEvent) {
  if (
    open.value &&
    !root.value?.contains(event.relatedTarget as Node | null)
  ) {
    closeMenu();
  }
}

onMounted(() => document.addEventListener("pointerdown", handlePointerDown));
onBeforeUnmount(() =>
  document.removeEventListener("pointerdown", handlePointerDown),
);
</script>

<template>
  <div
    ref="root"
    class="choice-select"
    @keydown="handleKeydown"
    @focusout="handleFocusOut"
  >
    <button
      ref="trigger"
      class="choice-trigger"
      type="button"
      :aria-label="`${label}，当前为${selectedLabel}`"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-controls="menuId"
      @click="open ? closeMenu() : openMenu()"
    >
      <span>{{ selectedLabel }}</span>
      <ChevronDown class="choice-chevron" :size="16" />
    </button>

    <Transition name="choice-menu">
      <div
        v-if="open"
        :id="menuId"
        ref="menu"
        class="choice-menu"
        role="listbox"
        :aria-label="label"
      >
        <button
          v-for="option in options"
          :key="String(option.value)"
          class="choice-option"
          type="button"
          role="option"
          tabindex="-1"
          :aria-selected="option.value === modelValue"
          @click="selectOption(option.value)"
        >
          <span>{{ option.label }}</span>
          <Check
            v-if="option.value === modelValue"
            class="choice-check"
            :size="15"
          />
        </button>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.choice-select {
  position: relative;
  flex: 0 0 auto;
  width: 130px;
}
.choice-trigger {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
  min-height: 42px;
  border: 1px solid #efcfbf;
  border-radius: 10px;
  padding: 0 11px;
  color: #624740;
  background: #fffaf7;
  font-size: 13px;
  text-align: left;
  transition:
    border-color 160ms ease,
    background-color 160ms ease,
    box-shadow 160ms ease;
}
.choice-trigger:hover {
  border-color: #e7b9a2;
  background: #fff3ec;
}
.choice-trigger:focus-visible {
  border-color: var(--theme-color);
  outline: none;
  box-shadow: 0 0 0 3px #f8d7c6;
}
.choice-chevron {
  flex: 0 0 auto;
  color: #a17367;
  transition: transform 160ms ease;
}
.choice-trigger[aria-expanded="true"] .choice-chevron {
  transform: rotate(180deg);
}
.choice-menu {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  z-index: 5;
  width: 100%;
  padding: 4px;
  border: 1px solid #efcfbf;
  border-radius: 12px;
  background: rgba(255, 250, 247, 0.98);
  box-shadow: 0 12px 28px rgba(91, 49, 39, 0.16);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
}
.choice-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
  min-height: 34px;
  border: 0;
  border-radius: 8px;
  padding: 0 8px;
  color: #69453d;
  background: transparent;
  font-size: 13px;
  text-align: left;
}
.choice-option:hover,
.choice-option:focus-visible {
  outline: none;
  background: #fff0e7;
}
.choice-option[aria-selected="true"] {
  color: var(--theme-color-dark);
  background: #ffe5d5;
  font-weight: 600;
}
.choice-check {
  flex: 0 0 auto;
}
.choice-menu-enter-active {
  transition:
    opacity 250ms ease-out,
    transform 250ms cubic-bezier(0.22, 1, 0.36, 1);
}
.choice-menu-leave-active {
  transition:
    opacity 200ms ease-in,
    transform 200ms ease-in;
}
.choice-menu-enter-from,
.choice-menu-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.98);
}
@media (max-width: 600px) {
  .choice-select {
    width: 116px;
  }
  .choice-trigger,
  .choice-option {
    font-size: 12px;
  }
  .choice-trigger {
    min-height: 38px;
    border-radius: 9px;
    padding: 0 10px;
  }
}
@media (prefers-reduced-motion: reduce) {
  .choice-trigger,
  .choice-chevron,
  .choice-menu-enter-active,
  .choice-menu-leave-active {
    transition: none;
  }
}
</style>
