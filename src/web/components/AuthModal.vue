<script setup lang="ts">
import { ChevronRight, KeyRound, LoaderCircle, X } from "lucide-vue-next";
import type { AuthStatus } from "../types";

defineProps<{ open: boolean; auth: AuthStatus; busy: boolean }>();
// 登录操作交由父级调用受控浏览器流程，组件本身不保存凭据。
const emit = defineEmits<{ close: []; login: []; logout: [] }>();
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('close')">
    <section class="modal glass">
      <button class="close-button" title="关闭" @click="emit('close')">
        <X :size="20" /></button
      ><span class="modal-icon"><KeyRound :size="22" /></span>
      <h2>SF 下载会话</h2>
      <template v-if="auth.authenticated"
        ><p>
          当前账号已通过官方登录验证。会话保存在此浏览器的本地安全 Cookie
          中，刷新页面或重启本地服务后仍可恢复。
        </p>
        <button class="text-button" @click="emit('logout')">
          退出当前会话 <ChevronRight :size="15" /></button></template
      ><template v-else
        ><p>
          将打开一个独立的官方登录窗口。账号、密码和滑块验证都只在该官方页面内完成，应用会自动取得本地下载会话。
        </p>
        <button
          class="primary-button full"
          :disabled="busy"
          @click="emit('login')"
        >
          <LoaderCircle v-if="busy" class="spin" :size="18" />{{
            busy ? "正在等待官方登录" : "打开官方登录窗口"
          }}
        </button></template
      >
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
.text-button {
  display: flex;
  align-items: center;
  border: 0;
  color: var(--theme-color-dark);
  background: none;
  font-weight: 600;
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
