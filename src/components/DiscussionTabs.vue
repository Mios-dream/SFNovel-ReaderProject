<script setup lang="ts">
import type { DiscussionTab } from "../composables/useBookDiscussions";

defineProps<{ modelValue: DiscussionTab }>();
defineEmits<{ "update:modelValue": [tab: DiscussionTab] }>();
</script>

<template>
  <nav class="discussion-tabs" role="tablist" aria-label="评论类型">
    <button
      type="button"
      role="tab"
      :aria-selected="modelValue === 'comments'"
      :class="{ active: modelValue === 'comments' }"
      @click="$emit('update:modelValue', 'comments')"
    >
      评论
    </button>
    <button
      type="button"
      role="tab"
      :aria-selected="modelValue === 'reviews'"
      :class="{ active: modelValue === 'reviews' }"
      @click="$emit('update:modelValue', 'reviews')"
    >
      书评
    </button>
  </nav>
</template>

<style scoped>
.discussion-tabs {
  display: inline-flex;
  gap: 2px;
  padding: 3px;
  border-radius: 11px;
  background: #f4f1eb;
}
.discussion-tabs button {
  min-width: 62px;
  height: 32px;
  padding: 0 14px;
  border: 0;
  border-radius: 8px;
  color: #8a8076;
  background: transparent;
  font: inherit;
  font-size: 13px;
  font-weight: 500;
  transition:
    color 160ms ease,
    background-color 160ms ease,
    box-shadow 160ms ease;
}
.discussion-tabs button.active {
  color: #4d4138;
  background: #fff;
  box-shadow: 0 1px 4px rgba(74, 59, 46, 0.12);
}
.discussion-tabs button:hover:not(.active) {
  color: #62574d;
}
@media (max-width: 900px) {
  .discussion-tabs {
    gap: 20px;
    padding: 0;
    border-radius: 0;
    background: transparent;
  }
  .discussion-tabs button {
    width: auto;
    min-width: 0;
    height: 36px;
    padding: 0;
    border-radius: 0;
    color: #8d8178;
    background: transparent;
    font-size: 15px;
  }
  .discussion-tabs button.active {
    color: #3f352f;
    background: transparent;
    box-shadow: none;
    font-weight: 600;
  }
}
@media (prefers-reduced-motion: reduce) {
  .discussion-tabs button {
    transition: none;
  }
}
</style>
