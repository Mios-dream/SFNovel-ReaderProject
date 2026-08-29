import { createApp } from "vue";
import App from "./App.vue";
import "./styles.css";

// 应用只有一个根组件，所有页面切换和弹窗状态由 App 统一协调。
createApp(App).mount("#app");
