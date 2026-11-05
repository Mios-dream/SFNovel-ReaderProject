<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { Heart, LoaderCircle, MessageCircle, X } from "@lucide/vue";
import type { BookComment, BookCommentReply, BookCommentReplyPage } from "../types";
import { nativeErrorMessage } from "../composables/deskShared";

const props = defineProps<{
  comment: BookComment;
  kind: "novel" | "audio" | "comic";
  reviews: boolean;
}>();
const emit = defineEmits<{ close: [] }>();

const replies = ref<BookCommentReply[]>([]);
const loading = ref(false);
const loadingMore = ref(false);
const error = ref("");
const page = ref(-1);
const hasMore = ref(false);
const avatarFailed = ref(false);
const failedReplies = reactive(new Set<number>());
let request = 0;

const initial = computed(() => props.comment.userName.trim().slice(0, 1) || "读");
const content = computed(() => cleanText(props.comment.content));
const createdAt = computed(() => formatDate(props.comment.createdAt, true));

function cleanText(value: string) {
  return value
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<\/p>/gi, "\n")
    .replace(/<[^>]+>/g, "")
    .replace(/\[em:[^\]]+\]/g, "")
    .replace(/[ \t]+/g, " ")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function formatDate(value: string, withYear = false) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("zh-CN", {
    year: withYear ? "numeric" : undefined,
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

async function load(reset = true) {
  const currentRequest = ++request;
  const currentPage = reset ? 0 : page.value + 1;
  if (reset) {
    loading.value = true;
    error.value = "";
  } else {
    loadingMore.value = true;
  }
  try {
    const result = await invoke<BookCommentReplyPage>("get_comment_replies", {
      kind: props.kind,
      commentId: props.comment.commentId,
      reviews: props.reviews,
      page: currentPage,
      size: 20,
    });
    if (currentRequest !== request) return;
    const known = new Set(replies.value.map((reply) => reply.replyId));
    replies.value = reset
      ? result.replies
      : [...replies.value, ...result.replies.filter((reply) => !known.has(reply.replyId))];
    page.value = result.page;
    hasMore.value = result.hasMore;
  } catch (cause) {
    if (currentRequest === request)
      error.value = nativeErrorMessage(cause, "回复加载失败");
  } finally {
    if (currentRequest === request) {
      loading.value = false;
      loadingMore.value = false;
    }
  }
}

function close() {
  emit("close");
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") close();
}

watch(
  () => props.comment.commentId,
  () => {
    replies.value = [];
    page.value = -1;
    hasMore.value = false;
    avatarFailed.value = false;
    failedReplies.clear();
    void load();
  },
  { immediate: true },
);

onMounted(() => window.addEventListener("keydown", handleKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", handleKeydown));
</script>

<template>
  <Teleport to="body">
    <Transition name="comment-detail">
      <div class="comment-detail-layer">
        <div class="comment-detail-backdrop" @click="close" />
        <aside
          class="comment-detail-panel"
          role="dialog"
          aria-modal="true"
          aria-labelledby="comment-detail-title"
        >
          <header class="detail-heading">
            <span id="comment-detail-title">讨论详情</span>
            <button type="button" title="关闭" @click="close">
              <X :size="18" />
            </button>
          </header>

          <div class="detail-scroll">
            <section class="detail-comment">
              <div class="detail-avatar">
                <span>{{ initial }}</span>
                <img
                  v-if="comment.avatar && !avatarFailed"
                  :src="comment.avatar"
                  :alt="`${comment.userName} 的头像`"
                  @error="avatarFailed = true"
                />
              </div>
              <div class="detail-author">
                <div>
                  <strong>{{ comment.userName }}</strong>
                  <span v-if="comment.roleName" class="detail-role">{{ comment.roleName }}</span>
                  <span v-if="comment.isStick" class="detail-role stick">置顶</span>
                </div>
                <time v-if="createdAt">{{ createdAt }}</time>
              </div>
              <h2 v-if="comment.title">{{ comment.title }}</h2>
              <p class="detail-content">{{ content }}</p>
              <footer class="detail-stats">
                <span><MessageCircle :size="15" />{{ comment.replyCount }} 条回复</span>
                <span><Heart :size="15" />{{ comment.likeCount }} 人喜欢</span>
              </footer>
            </section>

            <section class="replies-section" aria-labelledby="replies-title">
              <header class="replies-heading">
                <h3 id="replies-title">回复</h3>
                <span>{{ replies.length || comment.replyCount }}</span>
              </header>

              <div v-if="loading" class="replies-state">
                <LoaderCircle class="spin" :size="20" />正在读取回复
              </div>
              <div v-else-if="error && !replies.length" class="replies-state error">
                <span>{{ error }}</span>
                <button type="button" @click="load()">重试</button>
              </div>
              <div v-else-if="!replies.length" class="replies-state">
                <MessageCircle :size="20" />暂无回复
              </div>
              <template v-else>
                <div class="reply-list">
                  <article v-for="reply in replies" :key="reply.replyId" class="reply-item">
                    <div class="reply-avatar">
                      <span>{{ reply.userName.trim().slice(0, 1) || "读" }}</span>
                      <img
                        v-if="reply.avatar && !failedReplies.has(reply.replyId)"
                        :src="reply.avatar"
                        :alt="`${reply.userName} 的头像`"
                        @error="failedReplies.add(reply.replyId)"
                      />
                    </div>
                    <div class="reply-copy">
                      <header>
                        <strong>{{ reply.userName }}</strong>
                        <span v-if="reply.repliedUserName"
                          >回复 {{ reply.repliedUserName }}</span
                        >
                        <time>{{ formatDate(reply.createdAt) }}</time>
                      </header>
                      <p>{{ cleanText(reply.content) }}</p>
                      <span v-if="reply.likeCount" class="reply-like">
                        <Heart :size="13" />{{ reply.likeCount }}
                      </span>
                    </div>
                  </article>
                </div>
                <button
                  v-if="hasMore"
                  type="button"
                  class="load-more-replies"
                  :disabled="loadingMore"
                  @click="load(false)"
                >
                  <LoaderCircle v-if="loadingMore" class="spin" :size="16" />
                  {{ loadingMore ? "正在加载" : "加载更多回复" }}
                </button>
              </template>
            </section>
          </div>
        </aside>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.comment-detail-layer {
  position: fixed;
  z-index: 80;
  inset: 0;
  display: flex;
  justify-content: flex-end;
}
.comment-detail-backdrop {
  position: absolute;
  inset: 0;
  background: rgba(55, 39, 31, 0.27);
  backdrop-filter: blur(3px);
  -webkit-backdrop-filter: blur(3px);
}
.comment-detail-panel {
  position: relative;
  display: flex;
  width: min(520px, calc(100vw - 48px));
  height: calc(100dvh - 32px);
  flex-direction: column;
  margin: 16px 16px 16px 0;
  overflow: hidden;
  border: 1px solid rgba(239, 224, 213, 0.95);
  border-radius: 24px;
  background: #fffaf6;
  box-shadow: -18px 18px 56px rgba(66, 43, 31, 0.22);
}
.detail-heading {
  display: flex;
  min-height: 66px;
  align-items: center;
  justify-content: space-between;
  padding: 0 20px 0 24px;
  border-bottom: 1px solid #efe4db;
  color: #57473d;
  background: rgba(255, 250, 246, 0.92);
  font-size: 14px;
  font-weight: 600;
}
.detail-heading button {
  display: grid;
  width: 34px;
  height: 34px;
  padding: 0;
  border: 0;
  border-radius: 11px;
  color: #8e7a6e;
  background: #f4ebe4;
  place-items: center;
  transition:
    color 140ms ease,
    background-color 140ms ease;
}
.detail-heading button:hover {
  color: #a95d40;
  background: #f7e3d7;
}
.detail-scroll {
  min-height: 0;
  flex: 1;
  overflow-y: auto;
  padding: 28px 28px 34px;
}
.detail-comment {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr);
  gap: 14px;
}
.detail-avatar {
  position: relative;
  display: grid;
  width: 48px;
  height: 48px;
  grid-row: span 2;
  overflow: hidden;
  border-radius: 15px;
  color: #96563f;
  background:
    linear-gradient(145deg, rgba(255, 255, 255, 0.55), transparent 60%),
    #f2dfd3;
  font-size: 16px;
  font-weight: 600;
  place-items: center;
}
.detail-avatar span,
.detail-avatar img {
  grid-area: 1 / 1;
  width: 100%;
  height: 100%;
}
.detail-avatar img {
  object-fit: cover;
}
.detail-author {
  display: flex;
  min-width: 0;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.detail-author > div {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}
.detail-author strong {
  overflow: hidden;
  color: #514239;
  font-size: 14px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.detail-author time {
  flex: none;
  color: #aa998f;
  font-size: 12px;
}
.detail-role {
  padding: 2px 6px;
  border-radius: 5px;
  color: #8e786b;
  background: #f1eae4;
  font-size: 12px;
}
.detail-role.stick {
  color: #a55a3e;
  background: #f9e7dc;
}
.detail-comment h2 {
  grid-column: 2;
  margin: 18px 0 0;
  color: #4c3e35;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 22px;
  line-height: 1.45;
}
.detail-content {
  grid-column: 2;
  margin: 12px 0 0;
  color: #6f6259;
  font-size: 14px;
  line-height: 1.95;
  overflow-wrap: anywhere;
  white-space: pre-line;
}
.detail-stats {
  display: flex;
  grid-column: 2;
  flex-wrap: wrap;
  gap: 16px;
  margin-top: 18px;
  color: #a38b7e;
}
.detail-stats span {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
}
.replies-section {
  margin-top: 28px;
  padding-top: 22px;
  border-top: 1px solid #eee2d8;
}
.replies-heading {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}
.replies-heading h3 {
  margin: 0;
  color: #514239;
  font-size: 15px;
  font-weight: 600;
}
.replies-heading span {
  display: grid;
  min-width: 21px;
  height: 21px;
  padding: 0 6px;
  border-radius: 7px;
  color: #a5684e;
  background: #f7e7dd;
  font-size: 11px;
  place-items: center;
}
.replies-state {
  display: flex;
  min-height: 132px;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: #a38f83;
  font-size: 13px;
}
.replies-state.error {
  flex-direction: column;
  color: #ad6654;
  text-align: center;
}
.replies-state button,
.load-more-replies {
  min-height: 32px;
  padding: 0 12px;
  border: 1px solid #e9cbbb;
  border-radius: 9px;
  color: #a75b40;
  background: #fffdfa;
  font: inherit;
  font-size: 12px;
}
.reply-list {
  display: grid;
  gap: 2px;
}
.reply-item {
  display: grid;
  grid-template-columns: 34px minmax(0, 1fr);
  gap: 11px;
  padding: 14px 0;
}
.reply-item + .reply-item {
  border-top: 1px solid #f1e8e1;
}
.reply-avatar {
  position: relative;
  display: grid;
  width: 34px;
  height: 34px;
  overflow: hidden;
  border-radius: 11px;
  color: #9c5b42;
  background: #f3e2d7;
  font-size: 12px;
  font-weight: 600;
  place-items: center;
}
.reply-avatar span,
.reply-avatar img {
  grid-area: 1 / 1;
  width: 100%;
  height: 100%;
}
.reply-avatar img {
  object-fit: cover;
}
.reply-copy {
  min-width: 0;
}
.reply-copy header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}
.reply-copy header strong {
  color: #57483f;
  font-size: 13px;
  font-weight: 600;
}
.reply-copy header span {
  color: #a48b7d;
  font-size: 12px;
}
.reply-copy header time {
  margin-left: auto;
  color: #b3a39a;
  font-size: 12px;
}
.reply-copy p {
  margin: 6px 0 0;
  color: #74665d;
  font-size: 13px;
  line-height: 1.75;
  overflow-wrap: anywhere;
  white-space: pre-line;
}
.reply-like {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  margin-top: 7px;
  color: #ab8f80;
  font-size: 12px;
}
.load-more-replies {
  display: inline-flex;
  width: 100%;
  min-height: 38px;
  align-items: center;
  justify-content: center;
  gap: 7px;
  margin-top: 10px;
  border-radius: 11px;
}
.comment-detail-enter-active,
.comment-detail-leave-active {
  transition: opacity 220ms ease;
}
.comment-detail-enter-active .comment-detail-panel {
  transition: transform 300ms cubic-bezier(0.22, 1, 0.36, 1);
}
.comment-detail-leave-active .comment-detail-panel {
  transition: transform 180ms ease-in;
}
.comment-detail-enter-from,
.comment-detail-leave-to {
  opacity: 0;
}
.comment-detail-enter-from .comment-detail-panel,
.comment-detail-leave-to .comment-detail-panel {
  transform: translateX(28px);
}
@media (max-width: 900px) {
  .comment-detail-layer {
    align-items: flex-end;
  }
  .comment-detail-panel {
    width: 100%;
    height: min(88dvh, 760px);
    margin: 0;
    border-width: 1px 0 0;
    border-radius: 22px 22px 0 0;
    box-shadow: 0 -18px 52px rgba(66, 43, 31, 0.24);
  }
  .detail-scroll {
    padding: 22px 20px calc(26px + env(safe-area-inset-bottom));
  }
  .detail-comment {
    grid-template-columns: 42px minmax(0, 1fr);
    gap: 12px;
  }
  .detail-avatar {
    width: 42px;
    height: 42px;
    border-radius: 13px;
  }
  .comment-detail-enter-from .comment-detail-panel,
  .comment-detail-leave-to .comment-detail-panel {
    transform: translateY(26px);
  }
}
@media (prefers-reduced-motion: reduce) {
  .comment-detail-enter-active,
  .comment-detail-leave-active,
  .comment-detail-enter-active .comment-detail-panel,
  .comment-detail-leave-active .comment-detail-panel {
    transition: none;
  }
}
</style>
