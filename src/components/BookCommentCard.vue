<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Heart, MessageCircle } from "@lucide/vue";
import type { BookComment } from "../types";

const props = defineProps<{
  comment: BookComment;
  compact?: boolean;
}>();

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

watch(
  () => props.comment.commentId,
  () => {
    avatarFailed.value = false;
  },
);
</script>

<template>
  <article class="book-comment" :class="{ compact }">
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
        <span v-if="comment.roleName" class="comment-role">{{
          comment.roleName
        }}</span>
        <span v-if="comment.isStick" class="comment-stick">置顶</span>
        <time v-if="createdAt">{{ createdAt }}</time>
      </header>
      <h4 v-if="comment.title">{{ comment.title }}</h4>
      <p>{{ content }}</p>
      <footer v-if="comment.replyCount || comment.likeCount">
        <span v-if="comment.replyCount"
          ><MessageCircle :size="13" />{{ comment.replyCount }}</span
        >
        <span v-if="comment.likeCount"
          ><Heart :size="13" />{{ comment.likeCount }}</span
        >
      </footer>
    </div>
  </article>
</template>

<style scoped>
.book-comment {
  display: grid;
  min-width: 0;
  grid-template-columns: 38px minmax(0, 1fr);
  gap: 12px;
  padding: 16px 0;
}
.book-comment.compact {
  padding: 13px 0;
}
.comment-avatar {
  position: relative;
  display: grid;
  width: 38px;
  height: 38px;
  overflow: hidden;
  border-radius: 50%;
  color: #8c5a45;
  background: #f3e5db;
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
  color: #5c4d45;
  font-size: 13px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.comment-role,
.comment-stick {
  padding: 2px 6px;
  border-radius: 5px;
  color: #8d7569;
  background: #f2ede8;
  font-size: 12px;
}
.comment-stick {
  color: #a95c40;
  background: #f8e7dc;
}
.comment-meta time {
  margin-left: auto;
  color: #a79a91;
  font-size: 12px;
  white-space: nowrap;
}
.comment-copy h4 {
  margin: 7px 0 0;
  overflow-wrap: anywhere;
  color: #4f433b;
  font-size: 14px;
  font-weight: 600;
  line-height: 1.5;
}
.comment-copy p {
  margin: 7px 0 0;
  overflow-wrap: anywhere;
  color: #70645c;
  font-size: 13px;
  line-height: 1.72;
  white-space: pre-line;
}
.compact .comment-copy p {
  display: -webkit-box;
  overflow: hidden;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.comment-copy footer {
  display: flex;
  gap: 12px;
  margin-top: 8px;
  color: #a18476;
}
.comment-copy footer span {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
}
@media (max-width: 900px) {
  .book-comment {
    grid-template-columns: 36px minmax(0, 1fr);
    gap: 10px;
  }
  .comment-avatar {
    width: 36px;
    height: 36px;
  }
  .comment-copy p {
    font-size: 14px;
  }
}
</style>
