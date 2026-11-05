<script setup lang="ts">
import { RouterView } from "vue-router";
import AppOverlays from "./components/AppOverlays.vue";
import ToastHost from "./components/ToastHost.vue";
import { useDeskProvider } from "./desk/provider";

// 组成根：各域在这里创建并注入，页面通过 useAuth / useLibrary 等窄接口取用。
useDeskProvider();
</script>

<template>
  <RouterView />
  <AppOverlays />
  <ToastHost />
</template>
