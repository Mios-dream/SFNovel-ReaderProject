<script setup lang="ts">
import {
  CalendarCheck2,
  Check,
  Gift,
  LoaderCircle,
  Ticket,
  X,
} from "@lucide/vue";
import { computed, watch } from "vue";
import { useSignin } from "../desk";
import type { SigninPrize, SigninReward, SigninWeekItem } from "../types";
import ModalShell from "./ModalShell.vue";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ close: [] }>();

const isAndroid = /Android/i.test(navigator.userAgent);
const autoSigninTime = isAndroid ? "每 2 小时检查" : "应用运行时检查";

const {
  overview,
  loading,
  signing,
  claimingConfigId,
  autoSigninSaving,
  autoSigninEnabled,
  noticeText,
  isToday,
  canClaim,
  open: openSignin,
  submit,
  claim,
  toggleAutoSignin,
  openAppSettings,
  dismiss,
} = useSignin();

const week = computed(() => overview.value?.week.slice(0, 7) || []);
const todaySigned = computed(() =>
  week.value.some((item) => isToday(item.date) && item.isSignIn),
);
/**
 * 仅在 Android 上、自动签到已开启时显示提示。
 *
 * 厂商的自启动开关无法查询，所以这里不判断是否已授权，只固定给出一条说明，
 * 让用户在签到没按时执行时知道去哪里处理。
 */
const showBackgroundHint = computed(() => isAndroid && autoSigninEnabled.value);
const weekSignedCount = computed(
  () => week.value.filter((item) => item.isSignIn).length,
);
const claimableCount = computed(
  () => overview.value?.rewards.filter(canClaim).length || 0,
);
const weeklyReward = computed(() => {
  const rewards = overview.value?.rewards || [];
  return (
    rewards.find((reward) => reward.day === 7 && !reward.isFinish) ||
    rewards.find((reward) => reward.day === 7)
  );
});
const weeklyPrizes = computed(() => weeklyReward.value?.rewards || []);

function submitSignin() {
  void submit();
}

function claimReward(reward: SigninReward) {
  void claim(reward);
}

function weekItemToday(item: SigninWeekItem) {
  return isToday(item.date);
}

function rewardState(reward: SigninReward) {
  if (reward.isFinish || reward.status === 1) return "已领取";
  if (reward.status === 0) return "可领取";
  if (reward.status === -1) return "未达成";
  return "暂不可领";
}

function rewardTitle(reward: SigninReward) {
  return `${reward.typeName} ${reward.day}${reward.unit}`.trim();
}

function prizeLabel(prize: SigninPrize) {
  return `${prize.name} ${prize.num}`;
}

function prizeCount(prizes: SigninPrize[]) {
  return prizes.reduce((total, prize) => total + prize.num, 0);
}

function weekDayLabel(item: SigninWeekItem) {
  if (item.weekday) return item.weekday;
  const value = item.date.slice(0, 10);
  const date = new Date(`${value}T00:00:00Z`);
  if (Number.isNaN(date.getTime())) return "";
  return ["周日", "周一", "周二", "周三", "周四", "周五", "周六"][
    date.getUTCDay()
  ];
}

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) void openSignin();
    else dismiss();
  },
  { immediate: true },
);
</script>

<template>
  <ModalShell :open="open" @close="emit('close')">
    <section
      class="modal glass"
      role="dialog"
      aria-modal="true"
      aria-labelledby="signin-title"
    >
      <header class="modal-header">
        <div class="title-block">
          <span class="modal-icon"><CalendarCheck2 :size="22" /></span>
          <h2 id="signin-title">签到</h2>
        </div>
        <button class="icon-button" title="关闭" @click="emit('close')">
          <X :size="20" />
        </button>
      </header>

      <div class="modal-body">
        <div v-if="!overview" class="state-card" :aria-busy="loading">
          <LoaderCircle v-if="loading" class="spin" :size="22" />
          <CalendarCheck2 v-else :size="22" />
          <span>{{
            loading ? "正在读取签到信息" : "暂未读取到签到信息"
          }}</span>
        </div>

        <section v-else class="signin-card" aria-label="本周签到">
          <div class="summary">
            <div class="summary-stat">
              <span>连续签到</span>
              <strong
                >{{ overview.continueDays.totalContinueDays
                }}<small>天</small></strong
              >
            </div>
            <div class="summary-stat">
              <span>本周签到</span>
              <strong
                >{{ weekSignedCount
                }}<small>/{{ week.length }}</small></strong
              >
            </div>
            <button
              class="sign-button"
              :class="{ done: todaySigned }"
              :disabled="todaySigned || signing || loading"
              @click="submitSignin"
            >
              <LoaderCircle v-if="signing" class="spin" :size="17" />
              <Check v-else-if="todaySigned" :size="17" />
              <span>{{ todaySigned ? "已签到" : "签到" }}</span>
            </button>
          </div>
          <div v-if="showBackgroundHint" class="background-hint">
            <span class="background-hint-text">
              自动签到可能被系统清理，请允许自启动
            </span>
            <button
              type="button"
              class="background-hint-primary"
              @click="openAppSettings"
            >
              打开设置
            </button>
          </div>
          <div class="auto-signin">
            <div class="auto-signin-copy">
              <span class="auto-signin-label">自动签到</span>
              <span class="auto-signin-time">{{
                noticeText || autoSigninTime
              }}</span>
            </div>
            <button
              type="button"
              class="auto-signin-switch"
              role="switch"
              aria-label="自动签到"
              :aria-checked="autoSigninEnabled"
              :disabled="autoSigninSaving"
              @click="toggleAutoSignin"
            >
              <span class="auto-signin-thumb" />
            </button>
          </div>

          <div class="week-grid">
            <article
              v-for="item in week"
              :key="item.date || item.day"
              class="day-card"
              :class="{ signed: item.isSignIn, today: weekItemToday(item) }"
            >
              <span class="day-name">{{ weekDayLabel(item) }}</span>
              <span class="day-icon-wrap">
                <span class="day-icon" :class="{ signed: item.isSignIn }">
                  <Check
                    v-if="item.isSignIn"
                    :size="20"
                    :stroke-width="2.6"
                  />
                  <Ticket v-else :size="20" />
                </span>
                <span
                  v-if="!item.isSignIn && item.rewards.length"
                  class="day-count"
                >
                  {{ prizeCount(item.rewards) }}
                </span>
              </span>
            </article>

            <article
              class="day-card full-week"
              :class="{ complete: weekSignedCount === 7 }"
              aria-label="满签7天奖励"
            >
              <span class="day-name">满签7天</span>
              <span class="day-icon-wrap">
                <span class="gift-mark"><Gift :size="20" /></span>
                <span v-if="weeklyPrizes.length" class="day-count">
                  {{ prizeCount(weeklyPrizes) }}
                </span>
              </span>
            </article>
          </div>

          <section class="streak-rewards" aria-label="连签奖励">
            <div class="section-heading">
              <h3><Gift :size="16" />连签奖励</h3>
              <span v-if="claimableCount">{{ claimableCount }} 项可领取</span>
            </div>
            <div v-if="overview.rewards.length" class="reward-list">
              <article
                v-for="reward in overview.rewards"
                :key="reward.recordId || reward.configId"
                class="reward-item"
                :class="{ claimable: canClaim(reward) }"
              >
                <div class="reward-copy">
                  <strong>{{ rewardTitle(reward) }}</strong>
                  <div class="prize-list">
                    <span v-for="prize in reward.rewards" :key="prize.name">{{
                      prizeLabel(prize)
                    }}</span>
                  </div>
                </div>
                <button
                  v-if="canClaim(reward)"
                  class="claim-button"
                  :disabled="claimingConfigId !== undefined"
                  @click="claimReward(reward)"
                >
                  <LoaderCircle
                    v-if="claimingConfigId === reward.configId"
                    class="spin"
                    :size="15"
                  />
                  <span>领取</span>
                </button>
                <span v-else class="reward-state">{{
                  rewardState(reward)
                }}</span>
              </article>
            </div>
            <p v-else class="empty-rewards">暂无连签奖励</p>
          </section>
        </section>
      </div>
    </section>
  </ModalShell>
</template>

<style scoped>
.modal {
  display: flex;
  width: min(720px, 100%);
  max-height: min(880px, calc(100dvh - 36px));
  flex-direction: column;
  overflow: hidden;
  border-radius: 24px;
}
.modal-header {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 22px 22px 0;
}
.title-block {
  display: flex;
  align-items: center;
  gap: 12px;
}
.title-block h2 {
  margin: 0;
  font-size: 20px;
  line-height: 1.25;
}
.modal-icon {
  display: grid;
  width: 42px;
  height: 42px;
  flex: 0 0 auto;
  border-radius: 14px;
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 8px 18px #d9825440;
  place-items: center;
}
.icon-button {
  display: grid;
  width: 38px;
  height: 38px;
  border: 0;
  border-radius: 12px;
  color: #9a7569;
  background: transparent;
  transition:
    color 160ms ease,
    background 160ms ease;
  place-items: center;
}
.icon-button:hover {
  color: var(--theme-color-dark);
  background: #fbe9df;
}
.modal-body {
  min-height: 0;
  padding: 16px 22px 22px;
  overflow-y: auto;
}
.state-card {
  display: flex;
  min-height: 180px;
  align-items: center;
  justify-content: center;
  gap: 10px;
  border: 1px solid #f0ddd3;
  border-radius: 20px;
  color: #9a7a70;
  background: #fffaf7;
  font-size: 14px;
}
.signin-card {
  overflow: hidden;
  border: 1px solid #f0ddd3;
  border-radius: 20px;
  background: #fffaf7;
}
.summary {
  display: flex;
  align-items: center;
  gap: 18px;
  padding: 16px 18px;
  border-bottom: 1px solid #f4e6de;
}
.summary-stat {
  display: grid;
  gap: 2px;
}
.summary-stat + .summary-stat {
  padding-left: 18px;
  border-left: 1px solid #f2e3da;
}
.summary-stat > span {
  color: var(--muted);
  font-size: 12px;
}
.summary-stat strong {
  color: #674941;
  font-size: 20px;
  font-weight: 700;
  line-height: 1.15;
}
.summary-stat small {
  margin-left: 2px;
  color: #9a7a70;
  font-size: 12px;
  font-weight: 500;
}
.auto-signin {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 11px 18px;
  border-bottom: 1px solid #f4e6de;
  background: #fffdfb;
}
.auto-signin-copy {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}
.auto-signin-label {
  flex: 0 0 auto;
  color: #674941;
  font-size: 14px;
  font-weight: 600;
}
.auto-signin-time {
  min-width: 0;
  color: #a88377;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.background-hint {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 18px;
  border-bottom: 1px solid #f4e6de;
  background: #fff8e8;
}
.background-hint-text {
  flex: 1;
  min-width: 0;
  color: #8a6a3a;
  font-size: 12px;
  line-height: 1.4;
}
.background-hint-primary {
  flex: 0 0 auto;
  padding: 4px 12px;
  border: 0;
  border-radius: 999px;
  background: #e0902a;
  color: #fff;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}
.auto-signin-switch {
  position: relative;
  width: 40px;
  height: 24px;
  flex: 0 0 auto;
  padding: 0;
  border: 0;
  border-radius: 999px;
  background: #e7d8d1;
  transition: background 160ms ease;
}
.auto-signin-switch[aria-checked="true"] {
  background: var(--theme-color);
}
.auto-signin-switch:disabled {
  opacity: 0.6;
}
.auto-signin-switch:focus-visible {
  outline: 2px solid var(--theme-color);
  outline-offset: 2px;
}
.auto-signin-thumb {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 3px rgba(91, 49, 39, 0.18);
  transition: transform 160ms ease;
}
.auto-signin-switch[aria-checked="true"] .auto-signin-thumb {
  transform: translateX(16px);
}
.sign-button {
  display: inline-flex;
  min-width: 94px;
  min-height: 38px;
  align-items: center;
  justify-content: center;
  gap: 6px;
  margin-left: auto;
  padding: 0 14px;
  border: 0;
  border-radius: 11px;
  color: #fff;
  background: var(--theme-color);
  font-size: 14px;
  font-weight: 600;
  transition:
    background 160ms ease,
    transform 160ms ease;
}
.sign-button:hover:not(:disabled) {
  background: var(--theme-color-dark);
}
.sign-button:active:not(:disabled) {
  transform: scale(0.97);
}
.sign-button.done {
  color: var(--theme-color-dark);
  background: #fdece2;
}
.sign-button:disabled:not(.done) {
  cursor: default;
  opacity: 0.7;
}
.week-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
  padding: 14px 16px 16px;
}
.day-card {
  display: flex;
  min-width: 0;
  min-height: 100px;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 10px 10px 12px;
  border: 1px solid #f1e2da;
  border-radius: 14px;
  background: #fff;
  transition:
    background 160ms ease,
    border-color 160ms ease,
    box-shadow 160ms ease;
}
.day-name {
  color: #8f7168;
  font-size: 12px;
  line-height: 1.3;
  text-align: center;
  white-space: nowrap;
}
.day-icon-wrap {
  position: relative;
  display: grid;
  flex: 0 0 auto;
  place-items: center;
}
.day-icon {
  display: grid;
  width: 40px;
  height: 40px;
  flex: 0 0 auto;
  border-radius: 50%;
  color: var(--theme-color-dark);
  background: #fdece2;
  place-items: center;
}
.day-icon.signed {
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 4px 10px #e2946438;
}
.day-count {
  position: absolute;
  right: 2px;
  bottom: -5px;
  color: var(--theme-color-dark);
  font-size: 12px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  line-height: 1.25;
  text-align: center;
  -webkit-text-stroke: 2px #fff;
  paint-order: stroke fill;
}
.day-card.today {
  border-color: var(--theme-color);
  background: #fffaf7;
  box-shadow: inset 0 0 0 1px var(--theme-color);
}
.day-card.signed {
  border-color: #f0d3c2;
  background: #fff9f4;
}
.full-week {
  background: #fff8f1;
  border-color: #f1dccd;
}
.gift-mark {
  display: grid;
  width: 40px;
  height: 40px;
  flex: 0 0 auto;
  border-radius: 50%;
  color: var(--theme-color-dark);
  background: #ffe9dc;
  place-items: center;
}
.full-week.complete {
  border-color: #f0d3c2;
  background: #fff9f4;
}
.full-week.complete .gift-mark {
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 4px 10px #e2946438;
}
.streak-rewards {
  padding: 16px 16px 17px;
  border-top: 1px solid #f4e6de;
  background: #fff;
}
.section-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}
.section-heading h3 {
  display: flex;
  align-items: center;
  gap: 7px;
  margin: 0;
  color: #674941;
  font-size: 14px;
  font-weight: 600;
}
.section-heading > span {
  color: #a88377;
  font-size: 12px;
}
.reward-list {
  display: grid;
  gap: 6px;
}
.reward-item {
  display: flex;
  min-height: 52px;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 9px 10px 9px 12px;
  border: 1px solid transparent;
  border-radius: 12px;
  background: #fffaf7;
}
.reward-item.claimable {
  border-color: #f0cdb9;
  background: #fff7f1;
}
.reward-copy {
  min-width: 0;
}
.reward-copy strong {
  display: block;
  color: #674941;
  font-size: 14px;
  font-weight: 600;
}
.prize-list {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin-top: 4px;
}
.prize-list span {
  padding: 2px 7px;
  border-radius: 6px;
  color: #8d675c;
  background: #f7eae2;
  font-size: 12px;
}
.claim-button {
  display: inline-flex;
  min-width: 64px;
  min-height: 32px;
  align-items: center;
  justify-content: center;
  gap: 5px;
  flex: 0 0 auto;
  padding: 0 12px;
  border: 0;
  border-radius: 9px;
  color: #fff;
  background: var(--theme-color);
  font-size: 12px;
  font-weight: 600;
  transition:
    background 160ms ease,
    transform 160ms ease;
}
.claim-button:hover:not(:disabled) {
  background: var(--theme-color-dark);
}
.claim-button:active:not(:disabled) {
  transform: scale(0.97);
}
.claim-button:disabled {
  cursor: default;
  opacity: 0.65;
}
.reward-state {
  flex: 0 0 auto;
  color: #a28378;
  font-size: 12px;
}
.empty-rewards {
  margin: 0;
  color: var(--muted);
  font-size: 12px;
}
@media (max-width: 680px) {
  .modal {
    max-height: calc(100dvh - 20px);
    border-radius: 20px;
  }
  .modal-header {
    padding: 18px 16px 0;
  }
  .modal-body {
    padding: 12px 12px 16px;
  }
  .signin-card,
  .state-card {
    border-radius: 18px;
  }
  .summary {
    gap: 12px;
    padding: 14px;
  }
  .summary-stat + .summary-stat {
    padding-left: 12px;
  }
  .auto-signin {
    padding: 11px 14px;
  }
  .sign-button {
    min-width: 82px;
    min-height: 36px;
    padding: 0 12px;
  }
  .week-grid {
    gap: 6px;
    padding: 10px 10px 12px;
  }
  .day-card {
    min-height: 88px;
    gap: 5px;
    padding: 8px 6px 10px;
    border-radius: 12px;
  }
  .day-icon,
  .gift-mark {
    width: 34px;
    height: 34px;
  }
  .day-count {
    right: -4px;
    bottom: -4px;
  }
  .streak-rewards {
    padding: 14px 12px;
  }
}
@media (prefers-reduced-motion: reduce) {
  .icon-button,
  .sign-button,
  .claim-button,
  .day-card,
  .auto-signin-switch,
  .auto-signin-thumb {
    transition: none;
  }
}
</style>
