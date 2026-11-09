import { invoke } from "@tauri-apps/api/core";
import { computed, ref } from "vue";
import type { SigninNotice, SigninOverview, SigninReward } from "../../types";
import {
  beijingToday,
  calendarDateKey,
  isSameCalendarDay,
  type CalendarDate,
} from "../../utils/date";
import { nativeErrorMessage } from "../../utils/native";
import type { Notify } from "../../composables/useToast";
import type { SettingsDesk } from "./settings";

type SigninDeskOptions = {
  settings: SettingsDesk;
  notify: Notify;
};

/** 桌面端自动签到在应用运行时执行，已有应用内提示，不需要系统通知。 */
const isAndroid = /Android/i.test(navigator.userAgent);

/**
 * 签到与连签奖励。
 *
 * 自动签到开关的真相在设置域：这里只读取 `settings.settings` 并调用
 * `settings.setAutoSignin`，不另存一份布尔值，否则两个弹窗会各显示一个答案。
 */
export function createSigninDesk({ settings, notify }: SigninDeskOptions) {
  const overview = ref<SigninOverview>();
  const today = ref<CalendarDate>(beijingToday());
  const loading = ref(false);
  const signing = ref(false);
  const claimingConfigId = ref<number>();
  const autoSigninSaving = ref(false);
  const notice = ref<SigninNotice>();

  /** 请求代次：关闭弹窗后旧请求的结果不再回填。 */
  let generation = 0;

  const autoSigninEnabled = computed(
    () => settings.settings.value.signin.autoSigninEnabled,
  );

  /** 签到页展示的一行结论；不是当天的记录带上日期，避免误读。 */
  const noticeText = computed(() => {
    const value = notice.value;
    if (!value) return "";
    return isSameCalendarDay(value.day, today.value)
      ? value.message
      : `${value.day} ${value.message}`;
  });

  function isToday(value: string) {
    return isSameCalendarDay(value, today.value);
  }

  function canClaim(reward: SigninReward) {
    return reward.status === 0 && !reward.isFinish && reward.configId > 0;
  }

  async function refresh() {
    const request = ++generation;
    today.value = beijingToday();
    loading.value = true;
    try {
      const value = await invoke<SigninOverview>("get_signin_overview");
      if (request === generation) overview.value = value;
    } catch (error) {
      if (request === generation)
        notify(nativeErrorMessage(error, "读取签到信息失败"), "error");
    } finally {
      if (request === generation) loading.value = false;
    }
  }

  /** 读取最近一次自动签到结果，供签到页展示；不消费启动提示。 */
  async function loadNotice() {
    try {
      notice.value =
        (await invoke<SigninNotice | null>("signin_notice", {
          consume: false,
        })) ?? undefined;
    } catch {
      // 提示属于附加信息，读取失败不打扰用户。
    }
  }

  /** 启动时提示一次尚未展示的自动签到结果。 */
  async function announceAutoSignin() {
    try {
      const value = await invoke<SigninNotice | null>("signin_notice", {
        consume: true,
      });
      if (!value) return;
      notify(value.message, value.success ? "success" : "error");
    } catch {
      // 未登录或记录不可读时静默跳过。
    }
  }

  /**
   * 打开本应用的系统详情页。
   *
   * 自启动开关没有通用 Intent，位置也因厂商而异（vivo/小米/OPPO 在「权限」里，
   * 华为叫「应用启动管理」），所以只跳到详情页并提示用户自己找这个开关。
   * 是否已授权无法查询，因此这里只负责跳转，不做校验。
   */
  async function openAppSettings() {
    try {
      await invoke("open_app_settings");
      notify("请找到「自启动」并开启", "info");
    } catch (error) {
      notify(nativeErrorMessage(error, "打开应用设置失败"), "error");
    }
  }

  async function submit() {
    if (signing.value || loading.value) return;
    const date = calendarDateKey(today.value);
    if (overview.value?.week.some((item) => isToday(item.date) && item.isSignIn))
      return;
    signing.value = true;
    try {
      await invoke("submit_signin", { date });
      notify("签到成功", "success");
      await Promise.all([refresh(), loadNotice()]);
    } catch (error) {
      notify(nativeErrorMessage(error, "签到失败"), "error");
    } finally {
      signing.value = false;
    }
  }

  async function claim(reward: SigninReward) {
    if (!canClaim(reward) || claimingConfigId.value !== undefined) return;
    claimingConfigId.value = reward.configId;
    try {
      await invoke("claim_signin_reward", { configId: reward.configId });
      notify("奖励已领取", "success");
      await refresh();
    } catch (error) {
      notify(nativeErrorMessage(error, "领取签到奖励失败"), "error");
    } finally {
      claimingConfigId.value = undefined;
    }
  }

  /**
   * 等一次后台试签的结果。
   *
   * 打开开关后后端立刻试签，但要等网络往返与落盘，命令返回时结果还没写。
   * 这里只轮询本地记录（廉价），拿到当天成功记录后再刷新一次签到状态。
   */
  async function awaitAutoSigninResult() {
    for (let attempt = 0; attempt < 5; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 1_000));
      await loadNotice();
      if (notice.value?.success && isToday(notice.value.day)) break;
    }
    await refresh();
  }

  /**
   * 请求通知权限。
   *
   * 只在 Android 上需要：后台签到完成时用户看不到应用内提示。用户拒绝也不阻断
   * 自动签到，只是收不到结果提醒，所以这里只提示一次、不抛错。
   */
  async function requestNotificationPermission() {
    if (!isAndroid) return;
    try {
      const granted = await invoke<boolean>("request_notification_permission");
      if (!granted) notify("未授予通知权限，签到结果不会提醒", "info");
    } catch {
      // 通知属于附加能力，失败不影响签到本身。
    }
  }

  async function toggleAutoSignin() {
    if (autoSigninSaving.value) return;
    const next = !autoSigninEnabled.value;
    autoSigninSaving.value = true;
    try {
      // 先要权限再开启：试签在 setAutoSignin 之后立刻执行，晚一步通知就赶不上。
      if (next) await requestNotificationPermission();
      await settings.setAutoSignin(next);
      notify(
        next ? "已开启自动签到，正在试签今天" : "已关闭自动签到",
        "success",
      );
      if (next) void awaitAutoSigninResult();
    } catch (error) {
      notify(nativeErrorMessage(error, "更新自动签到设置失败"), "error");
    } finally {
      autoSigninSaving.value = false;
    }
  }

  /** 打开弹窗：先确保设置已加载（自动签到开关的真相），再读取签到状态。 */
  async function open() {
    await settings.ensureSettings().catch((error) => {
      notify(nativeErrorMessage(error, "读取自动签到设置失败"), "error");
    });
    await Promise.all([refresh(), loadNotice()]);
  }

  /** 关闭弹窗：作废进行中的请求，下次打开重新读取。 */
  function dismiss() {
    generation += 1;
    loading.value = false;
  }

  return {
    overview,
    today,
    loading,
    signing,
    claimingConfigId,
    autoSigninSaving,
    autoSigninEnabled,
    noticeText,
    isToday,
    canClaim,
    open,
    refresh,
    loadNotice,
    announceAutoSignin,
    submit,
    claim,
    toggleAutoSignin,
    openAppSettings,
    dismiss,
  };
}

/** 该域对外暴露的完整上下文；页面只应通过 `useSignin` 取用。 */
export type SigninDesk = ReturnType<typeof createSigninDesk>;
