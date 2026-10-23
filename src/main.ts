import { createApp } from "vue";
import App from "./App.vue";
import { reportError } from "./composables/useToast";
import { router } from "./router";
import "./styles.css";

const app = createApp(App);

app.config.errorHandler = (error, _instance, info) => {
  console.error(`[SF Novel Flow] Vue error: ${info}`, error);
  reportError(error, "页面出现异常，请稍后重试");
};

router.onError((error) => {
  reportError(error, "页面切换失败，请稍后重试");
});

window.addEventListener("error", (event) => {
  if (event instanceof ErrorEvent && (event.error || event.message)) {
    reportError(event.error || event.message, "应用运行异常，请稍后重试");
  }
});

window.addEventListener("unhandledrejection", (event) => {
  event.preventDefault();
  reportError(event.reason, "异步操作失败，请稍后重试");
});

app.use(router).mount("#app");
