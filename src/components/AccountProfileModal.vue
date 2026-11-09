<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  CalendarCheck2,
  CircleUserRound,
  Flame,
  Globe2,
  LoaderCircle,
  LogOut,
  Settings2,
  Smartphone,
  Ticket,
} from "@lucide/vue";
import type { AuthStatus, UserProfile } from "../types";
import ModalShell from "./ModalShell.vue";

const props = defineProps<{
  open: boolean;
  auth: AuthStatus;
  profile?: UserProfile;
  loading: boolean;
  error: string;
}>();
const emit = defineEmits<{
  close: [];
  logout: [];
  manageConnections: [];
  signin: [];
}>();
const avatarFailed = ref(false);

watch(
  () => props.profile?.avatar,
  () => {
    avatarFailed.value = false;
  },
);

const avatarLabel = computed(() => props.profile?.nickName.slice(0, 1) || "S");
/**
 * Formats a nullable account balance, using an explicit value for missing data.
 *
 * @param value The account value to display; missing values are shown as unknown.
 * @returns The locale-formatted number suitable for the profile template.
 */
function balance(value: number | null) {
  return value === null ? "未知" : new Intl.NumberFormat("zh-CN").format(value);
}
</script>

<template>
  <ModalShell :open="open" @close="emit('close')">
    <section class="modal glass" aria-labelledby="account-profile-title">
      <template v-if="loading && !profile">
        <div class="loading-state">
          <LoaderCircle class="spin" :size="26" /><span>正在读取账户信息</span>
        </div>
      </template>
      <template v-else-if="profile">
        <header class="profile-header">
          <div
            class="avatar"
            :class="{ fallback: !profile.avatar || avatarFailed }"
          >
            <img
              v-if="profile.avatar && !avatarFailed"
              :src="profile.avatar"
              :alt="`${profile.nickName} 的头像`"
              @error="avatarFailed = true"
            />
            <span v-else>{{ avatarLabel }}</span>
          </div>
          <div class="profile-copy">
            <div class="profile-tagline">
              <span class="account-tag">SF 账号</span>
              <button
                class="signin-button"
                :disabled="!auth.appAuthenticated"
                :title="
                  auth.appAuthenticated ? '每日签到' : '连接 App 登录后可签到'
                "
                @click="emit('signin')"
              >
                <CalendarCheck2 :size="15" />签到
              </button>
            </div>
            <h2 id="account-profile-title">{{ profile.nickName }}</h2>
          </div>
        </header>

        <div class="balance-grid" aria-label="账户余额">
          <article>
            <Flame :size="18" /><span>火券</span
            ><strong>{{ balance(profile.fireMoneyRemain) }}</strong>
          </article>
          <article>
            <Ticket :size="18" /><span>代券</span
            ><strong>{{ balance(profile.couponsRemain) }}</strong>
          </article>
          <article>
            <Ticket :size="18" /><span>月票</span
            ><strong>{{ balance(profile.monthlyTicket) }}</strong>
          </article>
        </div>
        <div v-if="profile.vipDetailsAvailable" class="membership">
          <CircleUserRound :size="17" /><span>VIP 等级</span
          ><strong>{{
            profile.vipSystem === "new"
              ? `VIP ${profile.vipLevel} / ${profile.vipName}`
              : `VIP ${profile.vipLevel}`
          }}</strong>
        </div>
        <div v-else class="app-details-hint">
          <Smartphone :size="17" />暂时无法读取 VIP 资料
        </div>
        <p v-if="error" class="error-message">{{ error }}</p>
      </template>
      <div v-else class="empty-state">
        <CircleUserRound :size="28" />
        <p>{{ error || "暂未获取到账号资料" }}</p>
      </div>
      <section class="credential-summary" aria-label="登录凭证状态">
        <strong>登录状态</strong>
        <div class="credential-icons">
          <span
            v-if="auth.appAuthenticated"
            class="credential-icon app"
            title="App 登录已连接"
            ><Smartphone :size="18"
          /></span>
          <span
            v-if="auth.webAuthenticated"
            class="credential-icon web"
            title="网站登录已连接"
            ><Globe2 :size="18"
          /></span>
        </div>
      </section>
      <div class="account-actions">
        <button class="connections-button" @click="emit('manageConnections')">
          <Settings2 :size="16" />管理登录
        </button>
        <button class="logout-button" @click="emit('logout')">
          <LogOut :size="16" />退出全部账户
        </button>
      </div>
    </section>
  </ModalShell>
</template>

<style scoped>
.modal {
  position: relative;
  width: min(430px, 100%);
  max-height: calc(100dvh - 36px);
  padding: 28px;
  overflow-y: auto;
  border-radius: 20px;
}
.profile-header {
  display: flex;
  align-items: center;
  gap: 13px;
  margin-bottom: 18px;
}
.avatar {
  display: grid;
  flex: 0 0 auto;
  width: 62px;
  height: 62px;
  overflow: hidden;
  border: 2px solid #fff;
  border-radius: 18px;
  background: #f7d5c3;
  box-shadow: 0 6px 16px #c77f5c35;
  place-items: center;
}
.avatar img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.avatar.fallback {
  color: #9d5540;
  font:
    27px KaTongFont,
    "Microsoft YaHei",
    sans-serif;
}
.profile-copy {
  flex: 1;
  min-width: 0;
}
.profile-tagline {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-height: 32px;
}
.account-tag {
  color: #b08072;
  font-size: 12px;
  white-space: nowrap;
}
.profile-header h2 {
  margin: 0;
  overflow: hidden;
  color: #69453d;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 23px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.balance-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}
.balance-grid article {
  display: grid;
  min-height: 90px;
  padding: 12px;
  border: 1px solid #f2dacd;
  border-radius: 10px;
  background: #fff9f5;
}
.balance-grid article svg {
  color: var(--theme-color-dark);
}
.balance-grid span {
  align-self: end;
  color: #a17a6e;
  font-size: 12px;
}
.balance-grid strong {
  overflow: hidden;
  color: #6f4940;
  font:
    19px KaTongFont,
    "Microsoft YaHei",
    sans-serif;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.membership {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 40px;
  margin-top: 10px;
  padding: 9px 12px;
  border-radius: 10px;
  color: #8a655b;
  background: #fff0e7;
  font-size: 12px;
}
.membership svg {
  color: var(--theme-color-dark);
}
.membership strong {
  margin-left: auto;
  color: #9a563f;
}
.app-details-hint {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 40px;
  margin-top: 10px;
  padding: 9px 12px;
  border: 1px dashed #efd0c0;
  border-radius: 10px;
  color: #8a655b;
  background: #fffaf7;
  font-size: 12px;
}
.app-details-hint svg {
  flex: 0 0 auto;
  color: #875a35;
}
.credential-summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 14px;
  min-height: 44px;
  padding: 0 12px;
  border: 1px solid #ecd8ce;
  border-radius: 10px;
  background: #fffaf7;
}
.credential-summary > strong {
  color: #7b564c;
  font-size: 13px;
}
.credential-icons {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 7px;
  min-height: 32px;
}
.credential-icon {
  display: grid;
  flex: 0 0 auto;
  width: 30px;
  height: 30px;
  border-radius: 8px;
  color: #967b70;
  background: #f4e7e1;
  place-items: center;
}
.credential-icon.app {
  color: #875a35;
  background: #fff0dc;
}
.credential-icon.web {
  color: #3f796b;
  background: #e3f4ed;
}
.signin-button {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  gap: 5px;
  min-height: 32px;
  padding: 0 10px;
  border: 1px solid #efc8b4;
  border-radius: 9px;
  color: var(--theme-color-dark);
  background: #fff4ec;
  font-size: 12px;
  font-weight: 600;
}
.signin-button:hover:not(:disabled) {
  border-color: var(--theme-color);
  background: #ffe9db;
}
.signin-button:disabled {
  cursor: not-allowed;
  border-color: #eadbd4;
  color: #b49b92;
  background: #f7efeb;
}
.account-actions {
  display: flex;
  gap: 8px;
  margin-top: 14px;
}
.connections-button,
.logout-button {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-width: 0;
  min-height: 40px;
  border-radius: 10px;
  font-size: 13px;
  font-weight: 600;
}
.connections-button {
  border: 1px solid #efd0c0;
  color: #8d5948;
  background: #fffaf7;
}
.connections-button:hover {
  border-color: var(--theme-color);
  color: var(--theme-color-dark);
  background: #fff0e7;
}
.logout-button {
  border: 1px solid #e7c8bd;
  color: #a76253;
  background: var(--theme-color);
}
.logout-button:hover {
  color: #fff;
  border-color: #c96f48;
  background: #c96f48;
}
.loading-state,
.empty-state {
  display: grid;
  min-height: 180px;
  color: #a27b6f;
  font-size: 13px;
  place-content: center;
  place-items: center;
  text-align: center;
}
.loading-state {
  gap: 10px;
}
.loading-state svg,
.empty-state svg {
  color: var(--theme-color);
}
.empty-state p {
  margin: 10px 0 0;
}
.error-message {
  margin: 12px 0 0;
  color: #b45c50;
  font-size: 12px;
  line-height: 1.5;
}
@media (max-width: 420px) {
  .modal {
    padding: 22px;
  }
  .profile-header {
    gap: 10px;
  }
  .signin-button {
    padding: 0 8px;
  }
  .balance-grid article {
    padding: 10px;
  }
  .balance-grid strong {
    font-size: 17px;
  }
}
</style>
