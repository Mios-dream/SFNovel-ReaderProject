<script setup lang="ts">
import { computed, reactive } from "vue";
import { Gift, Ticket } from "@lucide/vue";
import type { BookComment } from "../types";

const props = defineProps<{ items: BookComment[] }>();
const failedAvatars = reactive(new Set<number>());
const ticketGradients = [
  "linear-gradient(90deg, rgba(255, 255, 255, 0.18), transparent 40%), linear-gradient(90deg, #d8ab91 0%, #c89175 100%)",
  "linear-gradient(90deg, rgba(255, 255, 255, 0.2), transparent 40%), linear-gradient(90deg, #f1b170 0%, #e58b60 100%)",
  "linear-gradient(90deg, rgba(255, 255, 255, 0.2), transparent 42%), linear-gradient(90deg, #f29379 0%, #e56886 100%)",
  "linear-gradient(90deg, rgba(255, 255, 255, 0.24), transparent 42%), linear-gradient(90deg, #f8c95a 0%, #f18b5f 52%, #e96691 100%)",
];
const giftGradient =
  "linear-gradient(90deg, rgba(255, 255, 255, 0.24), transparent 42%), linear-gradient(90deg, #f8c95a 0%, #f2a05d 52%, #ee7b5c 100%)";
const phases = [0.08 + Math.random() * 0.34, 0.52 + Math.random() * 0.36];

const messages = computed(() =>
  props.items.map((comment) => {
    const kind = comment.extType === 2 ? "gift" : "ticket";
    const ticketNum = comment.ticketNum || 1;
    return {
      id: comment.commentId,
      avatar: comment.avatar,
      initial: comment.userName.trim().slice(0, 1) || "读",
      kind,
      gradient:
        kind === "gift"
          ? giftGradient
          : ticketGradients[
              ticketNum >= 100
                ? 3
                : ticketNum >= 10
                  ? 2
                  : ticketNum >= 5
                    ? 1
                    : 0
            ],
      text:
        kind === "gift"
          ? comment.content || `打赏了 ${ticketNum} 火券`
          : `${comment.userName} 投了 ${ticketNum} 张月票`,
    };
  }),
);
const rows = computed(() => {
  const result = [[], []] as (typeof messages.value)[];
  messages.value.forEach((message, index) => result[index % 2].push(message));
  if (result[1].length > 1) result[1].push(result[1].shift()!);
  if (!result[1].length) result[1].push(...result[0]);
  return result;
});
const duration = computed(() =>
  Math.max(14, Math.min(26, messages.value.length * 2.4)),
);
const rowStyles = computed(() =>
  phases.map((phase) => ({
    animationDuration: `${duration.value}s`,
    animationDelay: `-${(duration.value * phase).toFixed(1)}s`,
  })),
);

function markAvatarFailed(commentId: number) {
  failedAvatars.add(commentId);
}
</script>

<template>
  <div v-if="messages.length" class="discussion-danmaku" aria-label="读者互动">
    <div v-for="(row, rowIndex) in rows" :key="rowIndex" class="danmaku-row">
      <div class="danmaku-track" :style="rowStyles[rowIndex]">
        <div
          v-for="group in 6"
          :key="group"
          class="danmaku-group"
          :aria-hidden="group > 1"
        >
          <span
            v-for="message in row"
            :key="`${group}-${message.id}`"
            class="danmaku-item"
            :class="message.kind"
            :title="message.text"
            :style="{ background: message.gradient }"
          >
            <span class="danmaku-avatar">
              <span>{{ message.initial }}</span>
              <img
                v-if="message.avatar && !failedAvatars.has(message.id)"
                :src="message.avatar"
                alt=""
                @error="markAvatarFailed(message.id)"
              />
            </span>
            <span class="danmaku-message">
              <Gift v-if="message.kind === 'gift'" :size="12" />
              <Ticket v-else :size="12" />
              <span>{{ message.text }}</span>
            </span>
          </span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.discussion-danmaku {
  display: flex;
  flex-direction: column;
  gap: 7px;
  padding: 3px 0;
  mask-image: linear-gradient(
    90deg,
    transparent,
    #000 18px,
    #000 calc(100% - 18px),
    transparent
  );
  -webkit-mask-image: linear-gradient(
    90deg,
    transparent,
    #000 18px,
    #000 calc(100% - 18px),
    transparent
  );
}
.danmaku-row {
  height: 36px;
  overflow: hidden;
}
.danmaku-track {
  display: flex;
  width: max-content;
  height: 100%;
  align-items: center;
  animation: danmaku-scroll linear infinite;
  will-change: transform;
}
.danmaku-group {
  display: flex;
  flex: none;
  align-items: center;
  gap: 10px;
  padding-right: 10px;
}
.danmaku-item {
  display: inline-flex;
  height: 36px;
  align-items: center;
  gap: 8px;
  padding: 0 12px 0 5px;
  border: 0;
  border-radius: 999px;
  color: #fff;
  font-size: 13px;
  font-weight: 600;
  letter-spacing: 0.01em;
  /* text-shadow:
    0 1px 3px rgba(45, 20, 64, 0.52),
    0 0 10px rgba(45, 20, 64, 0.22); */
  white-space: nowrap;
}
.danmaku-avatar {
  position: relative;
  display: grid;
  width: 28px;
  height: 28px;
  flex: none;
  overflow: hidden;
  border: 2px solid rgba(255, 255, 255, 0.72);
  border-radius: 50%;
  color: #9a4228;
  background: #fff;
  font-size: 12px;
  font-weight: 700;
  place-items: center;
}
.danmaku-avatar span,
.danmaku-avatar img {
  grid-area: 1 / 1;
  width: 100%;
  height: 100%;
}
.danmaku-avatar img {
  object-fit: cover;
}
.danmaku-message {
  display: flex;
  max-width: min(320px, calc(100vw - 108px));
  min-width: 0;
  align-items: center;
  gap: 6px;
}
.danmaku-message span {
  overflow: hidden;
  text-overflow: ellipsis;
}
.danmaku-message svg {
  flex: 0 0 auto;
  opacity: 0.9;
}
.discussion-danmaku:hover .danmaku-track {
  animation-play-state: paused;
}
@keyframes danmaku-scroll {
  to {
    transform: translate3d(-16.6667%, 0, 0);
  }
}
@media (prefers-reduced-motion: reduce) {
  .discussion-danmaku {
    padding: 0;
    mask-image: none;
    -webkit-mask-image: none;
  }
  .danmaku-row {
    height: auto;
    overflow: visible;
  }
  .danmaku-track {
    animation: none;
    width: 100%;
  }
  .danmaku-group {
    flex-wrap: wrap;
    padding-right: 0;
  }
  .danmaku-group:not(:first-child) {
    display: none;
  }
}
@media (max-width: 900px) {
  .discussion-danmaku {
    gap: 6px;
  }
  .danmaku-row {
    height: 32px;
  }
  .danmaku-item {
    height: 32px;
    gap: 7px;
    padding: 0 12px 0 5px;
    font-size: 12px;
  }
  .danmaku-avatar {
    width: 24px;
    height: 24px;
  }
}
</style>
