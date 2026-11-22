<script setup lang="ts">
import {
  ArrowLeft,
  CircleCheck,
  ClipboardPaste,
  Copy,
  Globe2,
  KeyRound,
  LoaderCircle,
  LogOut,
  Smartphone,
  X,
} from "@lucide/vue";
import { computed, reactive, ref, watch } from "vue";
import type { AuthStatus, CredentialKind } from "../types";
import ModalShell from "./ModalShell.vue";

const props = defineProps<{
  open: boolean;
  auth: AuthStatus;
  /** 打开时落在哪个页签：网站凭据或 App 凭据。 */
  initialTab: CredentialKind;
  busy: boolean;
}>();
const emit = defineEmits<{
  close: [];
  login: [username: string, password: string];
  loginWithCookie: [payload: string];
  copyCredentials: [kind: CredentialKind];
  browserLogin: [];
  logout: [];
  logoutApp: [];
  logoutWeb: [];
}>();

const TAB_ORDER: CredentialKind[] = ["app", "web"];
const username = ref("");
const password = ref("");
/** 两个页签各自暂存粘贴内容，来回切换不会串。 */
const payloads = reactive<Record<CredentialKind, string>>({ web: "", app: "" });
const activeTab = ref<CredentialKind>("app");
/** 页签内是否切到「粘贴凭据」形态。 */
const pasting = ref(false);
/** 由当前面板量出来的高度，弹窗跟着它过渡。 */
const bodyHeight = ref<number>();

const tabPosition = computed(() => TAB_ORDER.indexOf(activeTab.value));
const connected = computed(() =>
  activeTab.value === "web"
    ? props.auth.webAuthenticated
    : props.auth.appAuthenticated,
);
/** 页签、形态、连接状态任一变化都换一张面板，统一走同一套过渡。 */
const panelKey = computed(() =>
  connected.value
    ? `${activeTab.value}-connected`
    : `${activeTab.value}-${pasting.value ? "paste" : "login"}`,
);

let observer: ResizeObserver | undefined;
let tracked: HTMLElement | undefined;

/**
 * 跟踪当前面板高度。
 *
 * 面板叠在同一个格子里淡入淡出，容器高度如果不显式给值，就会在两块内容之间
 * 取最大值再突跳；这里量出活动面板的真实高度写进容器，配合 CSS 过渡让弹窗
 * 平滑长缩。ResizeObserver 兜住字体加载、输入框手动拉伸这类非切换引起的变化。
 */
function trackPanel(element: unknown) {
  if (!(element instanceof HTMLElement)) return;
  tracked = element;
  bodyHeight.value = element.offsetHeight;
  if (!observer) {
    observer = new ResizeObserver(() => {
      if (tracked) bodyHeight.value = tracked.offsetHeight;
    });
  } else {
    observer.disconnect();
  }
  observer.observe(element);
}

function selectTab(tab: CredentialKind) {
  if (tab === activeTab.value) return;
  activeTab.value = tab;
  pasting.value = false;
}

/** 连接成功后退出粘贴形态，否则下次退出凭证会落回那张旧 JSON 表单。 */
watch(connected, (value) => {
  if (value) pasting.value = false;
});

/** 粘贴的凭据可能属于另一侧会话，desk 更新目标页签后跟着切过去。 */
watch(
  () => props.initialTab,
  (tab) => {
    if (props.open) selectTab(tab);
  },
);

watch(
  () => props.open,
  (open) => {
    if (!open) {
      tracked = undefined;
      observer?.disconnect();
      return;
    }
    // 重开时先交还自动高度，避免上一次面板的高度被当成过渡起点。
    bodyHeight.value = undefined;
    activeTab.value = props.initialTab;
    pasting.value = false;
  },
  { immediate: true },
);

function submitPassword() {
  emit("login", username.value.trim(), password.value);
}

/** 提交即清空输入框：凭据已经交给上层，留在这里只是重复内容。 */
function submitPayload() {
  const tab = activeTab.value;
  const payload = payloads[tab].trim();
  if (!payload) return;
  emit("loginWithCookie", payload);
  payloads[tab] = "";
}
</script>

<template>
  <ModalShell :open="open" @close="emit('close')">
    <section class="modal glass glass-flat">
      <button class="close-button" title="关闭" @click="emit('close')">
        <X :size="20" /></button
      ><span class="modal-icon"><KeyRound :size="22" /></span>
      <h2>SF 账号连接</h2>
      <p>App 与网站登录彼此独立，可单独连接。</p>
      <div
        class="login-tabs"
        :style="{ '--tab-index': String(tabPosition) }"
        role="tablist"
        aria-label="登录方式"
      >
        <span class="tab-indicator" aria-hidden="true"></span>
        <button
          class="login-tab"
          :class="{ active: activeTab === 'app' }"
          role="tab"
          :aria-selected="activeTab === 'app'"
          type="button"
          @click="selectTab('app')"
        >
          App 登录
        </button>
        <button
          class="login-tab"
          :class="{ active: activeTab === 'web' }"
          role="tab"
          :aria-selected="activeTab === 'web'"
          type="button"
          @click="selectTab('web')"
        >
          网站登录
        </button>
      </div>
      <div
        class="login-body"
        :style="bodyHeight ? { height: `${bodyHeight}px` } : undefined"
      >
        <Transition name="panel-fade" appear @enter="trackPanel">
          <div :key="panelKey" class="login-panel" role="tabpanel">
            <!-- App 登录 -->
            <template v-if="activeTab === 'app'">
              <div v-if="auth.appAuthenticated" class="connected-state">
                <span class="connection-icon app"
                  ><Smartphone :size="21"
                /></span>
                <div class="connection-copy">
                  <strong>App 登录已连接</strong>
                  <small>完整功能可用</small>
                </div>
                <CircleCheck class="connection-check" :size="20" />
                <button
                  class="credential-button"
                  type="button"
                  :disabled="busy"
                  @click="emit('copyCredentials', 'app')"
                >
                  <Copy :size="15" />复制登录凭据
                </button>
                <button
                  class="disconnect-button"
                  type="button"
                  @click="emit('logoutApp')"
                >
                  <LogOut :size="16" />退出 App 凭证
                </button>
              </div>
              <template v-else-if="pasting">
                <p>请粘贴复制的凭据Json</p>
                <form class="login-form" @submit.prevent="submitPayload">
                  <textarea
                    v-model="payloads.app"
                    autocomplete="off"
                    autocapitalize="off"
                    spellcheck="false"
                    rows="3"
                    placeholder='{ "cookie": "…", "deviceId": "…" }'
                    required
                  ></textarea>
                  <button
                    class="primary-button full"
                    type="submit"
                    :disabled="busy"
                  >
                    <LoaderCircle v-if="busy" class="spin" :size="18" />{{
                      busy ? "正在连接" : "连接登录"
                    }}
                  </button>
                </form>
                <button
                  class="toggle-button"
                  type="button"
                  @click="pasting = false"
                >
                  <ArrowLeft :size="14" />返回密码登录
                </button>
              </template>
              <template v-else>
                <p>连接 App 凭证获取完整功能，但可能触发风控。</p>
                <form class="login-form" @submit.prevent="submitPassword">
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
                  <button
                    class="primary-button full"
                    type="submit"
                    :disabled="busy"
                  >
                    <LoaderCircle v-if="busy" class="spin" :size="18" />{{
                      busy ? "正在连接" : "连接 App 登录"
                    }}
                  </button>
                </form>
                <button
                  class="toggle-button"
                  type="button"
                  @click="pasting = true"
                >
                  <ClipboardPaste :size="14" />通过Cookie登录
                </button>
              </template>
            </template>
            <!-- 网站登录 -->
            <template v-else>
              <div v-if="auth.webAuthenticated" class="connected-state">
                <span class="connection-icon web"><Globe2 :size="21" /></span>
                <div class="connection-copy">
                  <strong>网站登录已连接</strong>
                  <small>小说、书架等基础功能可用</small>
                </div>
                <CircleCheck class="connection-check" :size="20" />
                <button
                  class="credential-button"
                  type="button"
                  :disabled="busy"
                  @click="emit('copyCredentials', 'web')"
                >
                  <Copy :size="15" />复制登录凭据
                </button>
                <button
                  class="disconnect-button"
                  type="button"
                  @click="emit('logoutWeb')"
                >
                  <LogOut :size="16" />退出网站凭证
                </button>
              </div>
              <template v-else-if="pasting">
                <p>请粘贴复制的凭据Json</p>
                <form class="login-form" @submit.prevent="submitPayload">
                  <textarea
                    v-model="payloads.web"
                    autocomplete="off"
                    autocapitalize="off"
                    spellcheck="false"
                    rows="3"
                    placeholder='{ "cookie": "…", "deviceId": "…" }'
                    required
                  ></textarea>
                  <button
                    class="primary-button full"
                    type="submit"
                    :disabled="busy"
                  >
                    <LoaderCircle v-if="busy" class="spin" :size="18" />{{
                      busy ? "正在连接" : "连接登录"
                    }}
                  </button>
                </form>
                <button
                  class="toggle-button"
                  type="button"
                  @click="pasting = false"
                >
                  <ArrowLeft :size="14" />返回网站登录
                </button>
              </template>
              <template v-else>
                <p>用于小说，书架等基础功能的访问。</p>
                <button
                  class="primary-button full"
                  :disabled="busy"
                  @click="emit('browserLogin')"
                >
                  <LoaderCircle v-if="busy" class="spin" :size="18" />{{
                    busy ? "正在等待官方登录" : "连接网站登录"
                  }}
                </button>
                <button
                  class="toggle-button"
                  type="button"
                  @click="pasting = true"
                >
                  <ClipboardPaste :size="14" />通过Cookie登录
                </button>
              </template>
            </template>
          </div>
        </Transition>
      </div>
    </section>
  </ModalShell>
</template>

<style scoped>
.modal {
  position: relative;
  width: min(410px, 100%);
  max-height: calc(100dvh - 36px);
  padding: 28px;
  overflow-x: hidden;
  overflow-y: auto;
  border-radius: 20px;
  box-shadow: none;
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
  position: relative;
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  margin: 18px 0 15px;
  padding: 4px;
  border-radius: 12px;
  background: #f5e4db;
}
/* 选中底独立成一层：直接给按钮换背景是瞬切，平移过去才有「选中项在移动」的因果感。 */
.tab-indicator {
  position: absolute;
  top: 4px;
  bottom: 4px;
  left: 4px;
  width: calc(50% - 4px);
  border-radius: 9px;
  background: #fffaf7;
  box-shadow: 0 2px 6px rgba(117, 72, 56, 0.1);
  transform: translateX(calc(var(--tab-index, 0) * 100%));
  transition: transform 0.34s cubic-bezier(0.32, 0.72, 0.24, 1);
  will-change: transform;
}
.login-tab {
  position: relative;
  z-index: 1;
  min-height: 35px;
  padding: 0 8px;
  border: 0;
  border-radius: 9px;
  color: #a17367;
  background: transparent;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: color 0.24s ease;
}
.login-tab.active {
  color: var(--theme-color-dark);
}
/*
 * 进出场的面板叠在同一格里，容器高度由脚本量出的活动面板高度驱动：
 * 高度自己过渡，两块内容交替时才不会先塌再撑。
 */
.login-body {
  display: grid;
  align-items: start;
  /* clip 而不是 hidden：输入框的焦点环画在边框外 1px，hidden 会把它切掉。 */
  overflow: clip;
  overflow-clip-margin: 4px;
  transition: height 0.32s cubic-bezier(0.32, 0.72, 0.24, 1);
}
.login-body > * {
  grid-area: 1 / 1;
}
/* 纯淡入淡出：进场略慢于出场，旧内容先让开，新内容再接上。 */
.panel-fade-enter-active {
  transition: opacity 0.26s ease;
}
.panel-fade-leave-active {
  transition: opacity 0.2s ease;
}
.panel-fade-enter-from,
.panel-fade-leave-to {
  opacity: 0;
}
.login-panel p {
  margin-bottom: 14px;
}
.login-form {
  display: grid;
  gap: 9px;
}
.login-form input,
.login-form textarea {
  width: 100%;
  min-height: 40px;
  padding: 0 12px;
  border: 1px solid #efd8cc;
  border-radius: 10px;
  color: #69453d;
  background: rgba(255, 255, 255, 0.8);
  box-sizing: border-box;
  font-size: 16px;
}

.login-form textarea {
  min-height: 76px;
  max-height: 100px;
  padding: 9px 12px;
  font-family: inherit;
  line-height: 1.5;
  resize: vertical;
  word-break: break-all;
}

.login-form input:focus,
.login-form textarea:focus {
  outline: 1px solid var(--theme-color);
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
  flex: 1;
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
.credential-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
  min-height: 34px;
  border: 1px solid #efd1c2;
  border-radius: 8px;
  color: #9b5b4a;
  background: #fffaf7;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: 0.16s;
}
.credential-button:hover {
  border-color: #d79882;
  color: #7a4433;
}
.credential-button:disabled {
  cursor: wait;
  opacity: 0.6;
}
.disconnect-button {
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
.disconnect-button:hover {
  border-color: #d79882;
  color: #fff;
  background: #c96f48;
}
/* 卡片底部的轻量入口：不占视觉重心，点开才换成粘贴凭据的形态。 */
.toggle-button {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  margin: 10px auto 0;
  padding: 6px 10px;
  border: 0;
  border-radius: 8px;
  color: var(--muted);
  background: transparent;
  font-size: 12px;
  transition:
    color 0.2s ease,
    background 0.2s ease;
}
.toggle-button:hover {
  color: var(--theme-color-dark);
  background: rgba(255, 255, 255, 0.6);
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
