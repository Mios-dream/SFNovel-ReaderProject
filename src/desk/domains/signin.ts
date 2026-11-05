import { invoke } from "@tauri-apps/api/core";
import { computed, ref } from "vue";
import type { SigninOverview, SigninReward } from "../../types";
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

  /** 请求代次：关闭弹窗后旧请求的结果不再回填。 */
  let generation = 0;

  const autoSigninEnabled = computed(
    () => settings.settings.value.signin.autoSigninEnabled,
  );

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

  async function submit() {
    if (signing.value || loading.value) return;
    const date = calendarDateKey(today.value);
    if (overview.value?.week.some((item) => isToday(item.date) && item.isSignIn))
      return;
    signing.value = true;
    try {
      await invoke("submit_signin", { date });
      notify("签到成功", "success");
      await refresh();
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

  async function toggleAutoSignin() {
    if (autoSigninSaving.value) return;
    const next = !autoSigninEnabled.value;
    autoSigninSaving.value = true;
    try {
      await settings.setAutoSignin(next);
      notify(next ? "已开启自动签到" : "已关闭自动签到", "success");
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
    await refresh();
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
    isToday,
    canClaim,
    open,
    refresh,
    submit,
    claim,
    toggleAutoSignin,
    dismiss,
  };
}

/** 该域对外暴露的完整上下文；页面只应通过 `useSignin` 取用。 */
export type SigninDesk = ReturnType<typeof createSigninDesk>;
