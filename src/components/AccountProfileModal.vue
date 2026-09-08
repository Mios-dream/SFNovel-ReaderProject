<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  CircleUserRound,
  Coins,
  Flame,
  Globe2,
  LoaderCircle,
  LogOut,
  Settings2,
  Smartphone,
  Ticket,
  X,
} from "lucide-vue-next";
import type { AuthStatus, UserProfile } from "../types";

const props = defineProps<{
  open: boolean;
  auth: AuthStatus;
  profile?: UserProfile;
  loading: boolean;
  error: string;
}>();
const emit = defineEmits<{ close: []; logout: []; manageConnections: [] }>();
const avatarFailed = ref(false);

watch(
  () => props.profile?.avatar,
  () => {
    avatarFailed.value = false;
  },
);

const avatarLabel = computed(() => props.profile?.nickName.slice(0, 1) || "S");
/**
 * Formats a nullable numeric account value using the Chinese locale.
 *
 * @param value The account value to display; missing values are treated as zero.
 * @returns The locale-formatted number suitable for the profile template.
 */
const number = (value: number | undefined) =>
  new Intl.NumberFormat("zh-CN").format(value || 0);
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <section class="modal glass" aria-labelledby="account-profile-title">
      <button class="close-button" title="关闭" @click="emit('close')">
        <X :size="20" />
      </button>
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
          <div>
            <p>SF 账号</p>
            <h2 id="account-profile-title">{{ profile.nickName }}</h2>
            <small v-if="profile.accountId">ID: {{ profile.accountId }}</small>
          </div>
        </header>

        <div
          v-if="profile.webDetailsAvailable || profile.appDetailsAvailable"
          class="balance-grid"
          aria-label="账户余额"
        >
          <article>
            <Flame :size="18" /><span>火券</span
            ><strong>{{ number(profile.fireMoneyRemain) }}</strong>
          </article>
          <article>
            <Ticket :size="18" /><span>代券</span
            ><strong>{{ number(profile.couponsRemain) }}</strong>
          </article>
          <article v-if="profile.appDetailsAvailable">
            <Coins :size="18" /><span>金币</span
            ><strong>{{ number(profile.welfareCoin) }}</strong>
          </article>
          <article>
            <Ticket :size="18" /><span>月票</span
            ><strong>{{ number(profile.monthlyTicket) }}</strong>
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
        <strong>登录凭证</strong>
        <div class="credential-icons">
          <span
            v-if="auth.appAuthenticated"
            class="credential-icon app"
            title="App 高级能力已连接"
            ><Smartphone :size="18"
          /></span>
          <span
            v-if="auth.webAuthenticated"
            class="credential-icon web"
            title="网站功能已连接"
            ><Globe2 :size="18"
          /></span>
        </div>
      </section>
      <button class="connections-button" @click="emit('manageConnections')">
        <Settings2 :size="17" />管理登录连接
      </button>
      <button class="logout-button" @click="emit('logout')">
        <LogOut :size="17" />退出全部账户
      </button>
    </section>
  </div>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 10;
  display: grid;
  padding: 18px;
  background: rgba(91, 49, 39, 0.24);
  place-items: center;
}
.modal {
  position: relative;
  width: min(430px, 100%);
  padding: 28px;
  border-radius: 20px;
  background: rgba(255, 249, 245, 0.88);
}
.close-button {
  position: absolute;
  top: 16px;
  right: 16px;
  display: grid;
  width: 40px;
  height: 40px;
  border: 0;
  border-radius: 13px;
  color: #a17367;
  background: rgba(255, 255, 255, 0.7);
  place-items: center;
}
.close-button:hover {
  color: var(--theme-color-dark);
  background: #fff0e7;
}
.profile-header {
  display: flex;
  align-items: center;
  gap: 13px;
  margin-bottom: 23px;
  padding-right: 38px;
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
.profile-header p {
  margin: 0 0 3px;
  color: #b08072;
  font-size: 11px;
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
.profile-header small {
  color: #aa8275;
  font: 11px monospace;
}
.balance-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}
.balance-grid article {
  display: grid;
  min-height: 103px;
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
  font-size: 11px;
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
  margin-top: 12px;
  padding: 12px;
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
  margin-top: 12px;
  padding: 12px;
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
  margin-top: 18px;
  min-height: 48px;
  padding: 0 14px;
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
  width: 32px;
  height: 32px;
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
.connections-button,
.logout-button {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  width: 100%;
  min-height: 42px;
  margin-top: 22px;
  border: 1px solid #edc4b5;
  border-radius: 10px;
  color: #ad5b48;
  background: transparent;
  font-size: 13px;
  font-weight: 600;
}
.connections-button {
  margin-top: 22px;
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
  margin-top: 10px;
}
.logout-button:hover {
  color: #fff;
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
  .balance-grid article {
    padding: 10px;
  }
  .balance-grid strong {
    font-size: 17px;
  }
}
</style>
