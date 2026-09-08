<script setup lang="ts">
import {
  CircleCheck,
  Globe2,
  KeyRound,
  LoaderCircle,
  LogOut,
  Smartphone,
  X,
} from "lucide-vue-next";
import { ref, watch } from "vue";
import type { AuthStatus } from "../types";

const props = defineProps<{ open: boolean; auth: AuthStatus; busy: boolean }>();
const emit = defineEmits<{
  close: [];
  login: [username: string, password: string];
  browserLogin: [];
  logout: [];
  logoutApp: [];
  logoutWeb: [];
}>();
const username = ref("");
const password = ref("");
const activeTab = ref<"password" | "official">("official");
watch(
  () => props.open,
  (open) => {
    if (open)
      activeTab.value = props.auth.appAuthenticated ? "official" : "password";
  },
);
/**
 * Emits credentials for immediate native submission without retaining either field.
 * @returns No return value.
 */
/**
 * Emits the current credentials for immediate native authentication.
 *
 * The component does not persist either field; the parent forwards them to
 * Rust and clears the modal state after the native command completes.
 *
 * @returns No value; emits the `login` event with trimmed username and password.
 */
function submit() {
  const submittedUsername = username.value.trim();
  const submittedPassword = password.value;
  emit("login", submittedUsername, submittedPassword);
}
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <section class="modal glass">
      <button class="close-button" title="关闭" @click="emit('close')">
        <X :size="20" /></button
      ><span class="modal-icon"><KeyRound :size="22" /></span>
      <h2>SF 账号连接</h2>
      <p>提供两种登录凭证以使用全部应用功能</p>
      <div class="login-tabs" role="tablist" aria-label="登录方式">
        <button
          class="login-tab"
          :class="{ active: activeTab === 'official' }"
          role="tab"
          :aria-selected="activeTab === 'official'"
          type="button"
          @click="activeTab = 'official'"
        >
          官方网页登录
        </button>
        <button
          class="login-tab"
          :class="{ active: activeTab === 'password' }"
          role="tab"
          :aria-selected="activeTab === 'password'"
          type="button"
          @click="activeTab = 'password'"
        >
          账号密码
        </button>
      </div>
      <div v-if="activeTab === 'password'" class="login-panel" role="tabpanel">
        <div v-if="auth.appAuthenticated" class="connected-state">
          <span class="connection-icon app"><Smartphone :size="21" /></span>
          <div class="connection-copy">
            <strong>App 高级能力已连接</strong>
            <small>书架、账户资料和 App 限制章节可用</small>
          </div>
          <CircleCheck class="connection-check" :size="20" />
          <button
            class="disconnect-button"
            type="button"
            @click="emit('logoutApp')"
          >
            <LogOut :size="16" />退出 App 凭证
          </button>
        </div>
        <template v-else>
          <p>下载部分特殊的章节，但可能会导致账号风险，非必要不建议使用。</p>
          <form class="login-form" @submit.prevent="submit">
            <input
              v-model="username"
              autocomplete="username"
              placeholder="账号"
              required
            />
            <input
              v-model="password"
              autocomplete="current-password"
              placeholder="密码"
              type="password"
              required
            />
            <button class="primary-button full" type="submit" :disabled="busy">
              <LoaderCircle v-if="busy" class="spin" :size="18" />{{
                busy ? "正在连接" : "连接 App 高级能力"
              }}
            </button>
          </form>
        </template>
      </div>
      <div v-else class="login-panel" role="tabpanel">
        <div v-if="auth.webAuthenticated" class="connected-state">
          <span class="connection-icon web"><Globe2 :size="21" /></span>
          <div class="connection-copy">
            <strong>网站功能已连接</strong>
            <small>小说、有声、漫画与网页资源可用</small>
          </div>
          <CircleCheck class="connection-check" :size="20" />
          <button
            class="disconnect-button"
            type="button"
            @click="emit('logoutWeb')"
          >
            <LogOut :size="16" />退出网站凭证
          </button>
        </div>
        <template v-else>
          <p>
            用于基础功能和资源下载。将在应用内官方页面完成账号、密码和滑块验证。
          </p>
          <button
            class="primary-button full"
            :disabled="busy"
            @click="emit('browserLogin')"
          >
            <LoaderCircle v-if="busy" class="spin" :size="18" />{{
              busy ? "正在等待官方登录" : "连接网站功能"
            }}
          </button>
        </template>
      </div>
      <!-- <button
        v-if="auth.appAuthenticated && auth.webAuthenticated"
        class="logout-all-button"
        @click="emit('logout')"
      >
        退出全部登录会话
      </button> -->
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
  width: min(410px, 100%);
  padding: 28px;
  border-radius: 20px;
  background: rgba(255, 249, 245, 0.8);
}
.modal h2 {
  margin: 15px 0 5px;
  color: #69453d;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 23px;
}
.modal p {
  margin: 0 0 18px;
  color: #99766c;
  font-size: 12px;
  line-height: 1.7;
}
.login-tabs {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 4px;
  margin: 18px 0 15px;
  padding: 4px;
  border-radius: 12px;
  background: #f5e4db;
}
.login-tab {
  min-height: 35px;
  padding: 0 8px;
  border: 0;
  border-radius: 9px;
  color: #a17367;
  background: transparent;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}
.login-tab.active {
  color: var(--theme-color-dark);
  background: #fffaf7;
  box-shadow: 0 2px 6px rgba(117, 72, 56, 0.1);
}
.login-panel p {
  margin-bottom: 14px;
}
.login-form {
  display: grid;
  gap: 9px;
}
.login-form input {
  width: 100%;
  min-height: 39px;
  padding: 0 12px;
  border: 1px solid #efd8cc;
  border-radius: 10px;
  color: #69453d;
  background: rgba(255, 255, 255, 0.8);
  box-sizing: border-box;
}
.modal-icon {
  display: grid;
  width: 42px;
  height: 42px;
  border-radius: 13px;
  color: var(--theme-color-dark);
  background: #ffe5d5;
  place-items: center;
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
  transition: 0.2s;
}
.close-button:hover {
  color: var(--theme-color-dark);
  background: #fff0e7;
}
.connected-state {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  padding: 14px;
  border: 1px solid #efd1c2;
  border-radius: 10px;
  background: #fff7f1;
}
.connection-icon {
  display: grid;
  width: 40px;
  height: 40px;
  border-radius: 10px;
  place-items: center;
}
.connection-icon.app {
  color: #875a35;
  background: #fff0dc;
}
.connection-icon.web {
  color: #a6535c;
  background: #f8e5e4;
}
.connection-copy {
  flex: 1 1 190px;
  min-width: 0;
}
.connection-copy strong,
.connection-copy small {
  display: block;
}
.connection-copy strong {
  color: #765047;
  font-size: 13px;
}
.connection-copy small {
  margin-top: 3px;
  color: #a57a6e;
  font-size: 11px;
  line-height: 1.45;
}
.connection-check {
  flex: 0 0 auto;
  color: var(--theme-color-dark);
}
.disconnect-button,
.logout-all-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-height: 34px;
  border: 1px solid #e6c5b8;
  border-radius: 8px;
  color: #9b5b4a;
  background: #fffaf7;
  font-size: 12px;
  font-weight: 600;
}
.disconnect-button {
  width: 100%;
}
.disconnect-button:hover,
.logout-all-button:hover {
  border-color: #d79882;
  color: #fff;
  background: #c96f48;
}
.logout-all-button {
  width: 100%;
  margin-top: 16px;
}
.primary-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 39px;
  padding: 0 16px;
  border: 0;
  border-radius: 10px;
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 7px 12px #e2946440;
  font-size: 13px;
  font-weight: 600;
  transition: 0.2s;
}
.primary-button:hover {
  background: var(--theme-color-dark);
  transform: translateY(-1px);
}
.primary-button:disabled {
  cursor: wait;
  opacity: 0.7;
}
.full {
  width: 100%;
  margin-top: 8px;
}
</style>
