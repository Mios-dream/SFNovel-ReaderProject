<script setup lang="ts">
import { BookMarked, BookOpen, ChevronRight, CircleUserRound, Compass, LibraryBig, Sparkles } from "lucide-vue-next";
import type { AuthStatus, ViewName } from "../types";

defineProps<{ active: ViewName; bookshelfCount: number; libraryCount: number; auth: AuthStatus }>();
const emit = defineEmits<{ navigate: [view: ViewName]; login: [] }>();
</script>

<template>
  <aside class="sidebar glass">
    <div class="brand"><span class="brand-mark"><BookOpen :size="22" /></span><span>Novel Flow</span></div>
    <nav>
      <button :class="{ active: active === 'discover' }" @click="emit('navigate', 'discover')"><Compass :size="19" />发现小说</button>
      <button :class="{ active: active === 'bookshelf' }" @click="emit('navigate', 'bookshelf')"><BookMarked :size="19" />我的书架 <span v-if="bookshelfCount" class="count">{{ bookshelfCount }}</span></button>
      <button :class="{ active: active === 'library' }" @click="emit('navigate', 'library')"><LibraryBig :size="19" />本地书库 <span v-if="libraryCount" class="count">{{ libraryCount }}</span></button>
    </nav>
    <div class="sidebar-bottom">
      <div class="mini-card"><Sparkles :size="17" /><span>本地优先<br /><small>数据仅保存在此设备</small></span></div>
      <button class="account-button" @click="emit('login')"><CircleUserRound :size="20" /><span>{{ auth.authenticated ? auth.userName || '已登录 SF 账号' : '登录 SF 账号' }}</span><ChevronRight :size="16" /></button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar { position: relative; z-index: 1; display: flex; flex-direction: column; min-height: calc(100vh - 36px); padding: 22px 15px; border-radius: 22px; }
.brand { display: flex; align-items: center; gap: 10px; padding: 1px 8px 33px; color: var(--theme-color-dark); font-family: KaTongFont, "Microsoft YaHei", sans-serif; font-size: 21px; }
.brand-mark { display: grid; width: 36px; height: 36px; border-radius: 13px; color: #fff; background: var(--theme-color); box-shadow: 0 7px 14px #e2946440; place-items: center; }
nav { display: grid; gap: 7px; }
nav button, .account-button { display: flex; align-items: center; gap: 12px; padding: 11px 12px; border: 0; border-radius: 12px; color: #96746c; background: transparent; font-size: 14px; text-align: left; transition: 0.2s; }
nav button:hover, .account-button:hover { color: var(--theme-color-dark); background: #fff1e9; }
nav button.active { color: var(--theme-color-dark); background: #fbe0d0; box-shadow: inset 3px 0 0 var(--theme-color); font-weight: 600; }
.count { margin-left: auto; padding: 2px 6px; border-radius: 6px; color: #9b573e; background: #f5c1a5; font: 11px monospace; }
.sidebar-bottom { margin-top: auto; }
.mini-card { display: flex; gap: 9px; padding: 11px; border: 1px solid #f5d8c8; border-radius: 13px; color: #8b665d; background: rgba(255, 242, 233, 0.78); font-size: 12px; }
.mini-card svg { flex: 0 0 auto; color: var(--theme-color); }
.mini-card small { color: #b18a7d; font-size: 10px; }
.account-button { width: 100%; margin-top: 15px; padding: 8px 4px; }
.account-button span { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.account-button svg:last-child { margin-left: auto; }

@media (max-width: 760px) {
  .sidebar { display: flex; min-height: auto; margin-bottom: 10px; padding: 14px; border-radius: 17px; }
  .brand { padding: 0; }
  .sidebar nav, .sidebar-bottom .mini-card { display: none; }
  .sidebar-bottom { margin: 0 0 0 auto; }
  .account-button { margin: 0; padding: 0; }
  .account-button span { display: none; }
}
</style>
