<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ChevronRight, Heart, MessageCircle } from "@lucide/vue";
import type { BookComment } from "../types";

const props = defineProps<{
  comment: BookComment;
  compact?: boolean;
  interactive?: boolean;
  selected?: boolean;
}>();
const emit = defineEmits<{ select: [comment: BookComment] }>();

const avatarFailed = ref(false);
const initial = computed(() => props.comment.userName.trim().slice(0, 1) || "读");
const content = computed(() =>
  props.comment.content
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<\/p>/gi, "\n")
    .replace(/<[^>]+>/g, "")
    .replace(/\[em:[^\]]+\]/g, "")
    .replace(/[ \t]+/g, " ")
    .replace(/\n{3,}/g, "\n\n")
    .trim(),
);
const createdAt = computed(() => {
  if (!props.comment.createdAt) return "";
  const date = new Date(props.comment.createdAt);
  return Number.isNaN(date.getTime())
    ? props.comment.createdAt
    : new Intl.DateTimeFormat("zh-CN", {
        month: "numeric",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      }).format(date);
});

function select() {
  if (props.interactive) emit("select", props.comment);
}

watch(
  () => props.comment.commentId,
  () => {
    avatarFailed.value = false;
  },
);
</script>

<template>
  <article
    class="book-comment"
    :class="{ compact, interactive, selected }"
    :role="interactive ? 'button' : undefined"
    :tabindex="interactive ? 0 : undefined"
    @click="select"
    @keydown.enter="select"
    @keydown.space.prevent="select"
  >
    <div class="comment-avatar">
      <span>{{ initial }}</span>
      <img
        v-if="comment.avatar && !avatarFailed"
        :src="comment.avatar"
        :alt="`${comment.userName} 的头像`"
        @error="avatarFailed = true"
      />
    </div>
    <div class="comment-copy">
      <header class="comment-meta">
        <strong>{{ comment.userName }}</strong>
        <span v-if="comment.roleName" class="comment-role">{{ comment.roleName }}</span>
        <span v-if="comment.isStick" class="comment-stick">置顶</span>
        <time v-if="createdAt">{{ createdAt }}</time>
      </header>
      <h4 v-if="comment.title">{{ comment.title }}</h4>
      <p>{{ content }}</p>
      <footer v-if="comment.replyCount || comment.likeCount || interactive">
        <span v-if="comment.replyCount" class="comment-stat">
          <MessageCircle :size="13" />{{ comment.replyCount }}
        </span>
        <span v-if="comment.likeCount" class="comment-stat">
          <Heart :size="13" />{{ comment.likeCount }}
        </span>
        <span v-if="interactive" class="comment-open" aria-hidden="true">
          <ChevronRight :size="16" />
        </span>
      </footer>
    </div>
  </article>
</template>

<style scoped>
.book-comment {
  position: relative;
  display: grid;
  min-width: 0;
  align-self: start;
  grid-template-columns: 40px minmax(0, 1fr);
  gap: 13px;
  padding: 18px;
  border: 1px solid #eee3d9;
  border-radius: 16px;
  background: rgba(255, 254, 251, 0.86);
  transition:
    border-color 160ms ease,
    background-color 160ms ease,
    box-shadow 180ms ease,
    transform 180ms ease;
}
.book-comment.interactive {
  cursor: pointer;
  outline: none;
}
.book-comment.interactive:hover {
  border-color: #e6c8b8;
  background: #fffdfa;
  box-shadow: 0 9px 24px rgba(102, 67, 48, 0.08);
  transform: translateY(-1px);
}
.book-comment.interactive:focus-visible {
  border-color: #df9f7d;
  box-shadow: 0 0 0 3px rgba(226, 148, 100, 0.2);
}
.book-comment.selected {
  border-color: #e2a27f;
  background: #fff8f3;
  box-shadow: 0 0 0 3px rgba(226, 148, 100, 0.14);
}
.book-comment.compact {
  padding: 15px;
  border-radius: 14px;
}
.comment-avatar {
  position: relative;
  display: grid;
  width: 40px;
  height: 40px;
  overflow: hidden;
  border-radius: 13px;
  color: #9c5b42;
  background:
    linear-gradient(145deg, rgba(255, 255, 255, 0.5), transparent 58%),
    #f3e2d7;
  box-shadow: inset 0 0 0 1px rgba(196, 132, 102, 0.1);
  font-size: 14px;
  font-weight: 600;
  place-items: center;
}
.comment-avatar span,
.comment-avatar img {
  grid-area: 1 / 1;
  width: 100%;
  height: 100%;
}
.comment-avatar img {
  object-fit: cover;
}
.comment-copy {
  min-width: 0;
}
.comment-meta {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}
.comment-meta strong {
  max-width: 100%;
  overflow: hidden;
  color: #57483f;
  font-size: 13px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.comment-role,
.comment-stick {
  padding: 2px 6px;
  border-radius: 5px;
  color: #8b7569;
  background: #f3ede7;
  font-size: 12px;
  line-height: 1.4;
}
.comment-stick {
  color: #a55a3e;
  background: #f9e7dc;
}
.comment-meta time {
  margin-left: auto;
  color: #ac9d94;
  font-size: 12px;
  white-space: nowrap;
}
.comment-copy h4 {
  margin: 8px 0 0;
  overflow-wrap: anywhere;
  color: #4f4239;
  font-size: 14px;
  font-weight: 600;
  line-height: 1.55;
}
.comment-copy p {
  display: -webkit-box;
  margin: 7px 0 0;
  overflow: hidden;
  overflow-wrap: anywhere;
  color: #74675f;
  font-size: 13px;
  line-height: 1.78;
  white-space: pre-line;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.comment-copy footer {
  display: flex;
  min-height: 18px;
  align-items: center;
  gap: 13px;
  margin-top: 11px;
  color: #a0877a;
}
.comment-stat {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
}
.comment-open {
  display: inline-flex;
  align-items: center;
  margin-left: auto;
  color: #c2afa3;
  transition:
    color 160ms ease,
    transform 160ms ease;
}
.book-comment.interactive:hover .comment-open,
.book-comment.selected .comment-open {
  color: #ae6548;
  transform: translateX(2px);
}
@media (max-width: 900px) {
  .book-comment {
    align-self: stretch;
    grid-template-columns: 36px minmax(0, 1fr);
    gap: 10px;
    padding: 16px 0;
    border: 0;
    border-radius: 0;
    background: transparent;
    box-shadow: none;
    transform: none;
  }
  .book-comment.interactive:hover,
  .book-comment.selected {
    background: transparent;
    box-shadow: none;
    transform: none;
  }
  .book-comment.interactive:focus-visible {
    border-radius: 0;
    outline: 2px solid rgba(226, 148, 100, 0.5);
    outline-offset: 2px;
    box-shadow: none;
  }
  .book-comment.compact {
    padding: 16px 0;
    border-radius: 0;
  }
  .comment-avatar {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: #f3e5db;
    box-shadow: none;
  }
  .comment-copy p {
    font-size: 14px;
  }
  .comment-copy footer {
    min-height: 0;
    margin-top: 8px;
  }
  .comment-open {
    display: none;
  }
}
@media (prefers-reduced-motion: reduce) {
  .book-comment,
  .comment-open {
    transition: none;
  }
  .book-comment.interactive:hover {
    transform: none;
  }
}
</style>
