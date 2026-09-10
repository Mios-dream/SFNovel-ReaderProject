<script setup lang="ts">
import {
  BookMarked,
  BookOpen,
  ChevronRight,
  CircleUserRound,
  Compass,
  LibraryBig,
  Settings2,
  Sparkles,
} from "lucide-vue-next";
import { ref, watch } from "vue";
import type { AuthStatus, UserProfile, ViewName } from "../types";

const props = defineProps<{
  active: ViewName;
  bookshelfCount: number;
  libraryCount: number;
  auth: AuthStatus;
  profile?: UserProfile;
}>();
// 侧栏仅负责导航和登录入口，页面数据通过 props 展示。
const emit = defineEmits<{
  navigate: [view: ViewName];
  account: [];
  requestSettings: [];
}>();
const avatarFailed = ref(false);

watch(
  () => props.profile?.avatar,
  () => {
    avatarFailed.value = false;
  },
);
</script>

<template>
  <aside class="sidebar glass">
    <div class="brand">
      <span class="brand-mark"><BookOpen :size="22" /></span
      ><span>SF Novel Flow</span>
    </div>
    <nav>
      <button
        :class="{ active: active === 'discover' }"
        @click="emit('navigate', 'discover')"
      >
        <Compass :size="19" />发现小说
      </button>
      <button
        :class="{ active: active === 'bookshelf' }"
        @click="emit('navigate', 'bookshelf')"
      >
        <BookMarked :size="19" />我的书架
        <span v-if="bookshelfCount" class="count">{{ bookshelfCount }}</span>
      </button>
      <button
        :class="{ active: active === 'library' }"
        @click="emit('navigate', 'library')"
      >
        <LibraryBig :size="19" />本地书库
        <span v-if="libraryCount" class="count">{{ libraryCount }}</span>
      </button>
    </nav>
    <div class="sidebar-bottom">
      <div class="mini-card">
        <Sparkles :size="17" /><span
          >本地优先<br /><small>数据仅保存在此设备</small></span
        >
      </div>
      <button
        class="request-settings-button"
        title="设置"
        @click="emit('requestSettings')"
      >
        <Settings2 :size="20" /><span>应用设置</span>
      </button>
      <button class="account-button" @click="emit('account')">
        <span
          v-if="auth.authenticated && profile?.avatar && !avatarFailed"
          class="account-avatar"
        >
          <img
            :src="profile.avatar"
            :alt="`${profile.nickName} 的头像`"
            @error="avatarFailed = true"
          />
        </span>
        <CircleUserRound v-else :size="20" /><span>{{
          profile?.nickName ||
          (auth.authenticated ? "已登录 SF 账号" : "登录 SF 账号")
        }}</span
        ><ChevronRight :size="16" />
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  position: sticky;
  top: 18px;
  z-index: 1;
  display: flex;
  flex-direction: column;
  height: calc(100vh - 36px);
  min-height: 0;
  padding: 22px 15px;
  border-radius: 22px;
}
.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 1px 8px 33px;
  color: var(--theme-color-dark);
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 21px;
}
.brand-mark {
  display: grid;
  width: 36px;
  height: 36px;
  border-radius: 13px;
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 7px 14px #e2946440;
  place-items: center;
}
nav {
  display: grid;
  gap: 7px;
}
nav button,
.account-button {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 11px 12px;
  border: 0;
  border-radius: 12px;
  color: #96746c;
  background: transparent;
  font-size: 14px;
  text-align: left;
  transition: 0.2s;
}
nav button:hover,
.account-button:hover {
  color: var(--theme-color-dark);
  background: #fff1e9;
}
nav button.active {
  color: var(--theme-color-dark);
  background: #fbe0d0;
  box-shadow: inset 3px 0 0 var(--theme-color);
  font-weight: 600;
}
.count {
  margin-left: auto;
  padding: 2px 6px;
  border-radius: 6px;
  color: #9b573e;
  background: #f5c1a5;
  font: 11px monospace;
}
.sidebar-bottom {
  margin-top: auto;
}
.mini-card {
  display: flex;
  gap: 9px;
  padding: 11px;
  border: 1px solid #f5d8c8;
  border-radius: 13px;
  color: #8b665d;
  background: rgba(255, 242, 233, 0.78);
  font-size: 12px;
}
.mini-card svg {
  margin: auto 0;
  flex: 0 0 auto;
  color: var(--theme-color);
}
.mini-card small {
  color: #b18a7d;
  font-size: 10px;
}
.account-button {
  width: 100%;
  margin-top: 15px;
  padding: 8px 4px;
}
.account-button span {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.request-settings-button {
  display: flex;
  width: 100%;
  align-items: center;
  justify-content: center;
  gap: 8px;
  margin-top: 12px;
  padding: 10px 12px;
  border: 1px solid #f1c8b3;
  border-radius: 12px;
  color: #9b573e;
  background: #fff7f2;
  font-size: 14px;
}
.request-settings-button:hover {
  color: #fff;
  background: var(--theme-color);
}
.account-button .account-avatar {
  display: block;
  flex: 0 0 auto;
  width: 20px;
  height: 20px;
  overflow: hidden;
  border-radius: 50%;
  background: #f5d8c8;
}
.account-avatar img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.account-button svg:last-child {
  margin-left: auto;
}

@media (max-width: 760px) {
  .sidebar {
    position: static;
    top: auto;
    display: flex;
    flex-direction: row;
    height: auto;
    min-height: auto;
    margin-bottom: 0;
    padding: 6px 2px;
    border: 0;
    border-radius: 0;
    background: transparent;
    box-shadow: none;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }
  .brand {
    gap: 8px;
    padding: 0;
    font-size: 18px;
  }
  .brand-mark {
    width: 34px;
    height: 34px;
    border-radius: 10px;
  }
  .sidebar-bottom .mini-card {
    display: none;
  }
  .sidebar nav {
    position: fixed;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 8;
    display: flex;
    gap: 2px;
    min-height: calc(64px + env(safe-area-inset-bottom));
    padding: 7px 12px calc(7px + env(safe-area-inset-bottom));
    border-top: 1px solid rgba(224, 176, 154, 0.6);
    background: #fff9f5;
    box-shadow: 0 -8px 24px rgba(137, 76, 55, 0.1);
  }
  .sidebar nav button {
    flex: 1 1 0;
    flex-direction: column;
    justify-content: center;
    gap: 2px;
    min-height: 50px;
    padding: 4px 6px;
    border-radius: 8px;
    font-size: 11px;
    text-align: center;
  }
  .sidebar nav button.active {
    box-shadow: none;
  }
  .sidebar nav .count {
    position: absolute;
    top: 5px;
    margin-left: 30px;
    min-width: 17px;
    padding: 1px 4px;
    font-size: 9px;
  }
  .sidebar-bottom {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 0 0 auto;
  }
  .request-settings-button {
    display: grid;
    width: 40px;
    height: 40px;
    margin: 0;
    padding: 0;
    border: 0;
    border-radius: 12px;
    color: #a17367;
    background: rgba(255, 255, 255, 0.58);
    place-items: center;
  }
  .request-settings-button span {
    display: none;
  }
  .account-button {
    width: 40px;
    height: 40px;
    margin: 0;
    padding: 0;
    border-radius: 12px;
    justify-content: center;
    background: rgba(255, 255, 255, 0.58);
  }
  .account-button span {
    display: none;
  }
  .account-button svg:last-child {
    display: none;
  }
}
</style>
