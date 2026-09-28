<script setup lang="ts">
import {
  computed,
  inject,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { useRouter } from "vue-router";
import {
  ChevronRight,
  ChevronLeft,
  Check,
  Headphones,
  ListMusic,
  Pause,
  Play,
  SkipBack,
  SkipForward,
  SlidersHorizontal,
  Timer,
  TimerOff,
  Volume2,
  X,
} from "lucide-vue-next";
import AudioPlayerDrawer from "../components/AudioPlayerDrawer.vue";
import { deskInjectionKey } from "../deskContext";

function requireDesk() {
  const desk = inject(deskInjectionKey);
  if (!desk) throw new Error("Desk context is unavailable");
  return desk;
}

const AUDIO_PLAYER_PREFERENCES_KEY = "sf-novel-flow.audio-player-preferences";
const playbackRates = [1, 1.25, 1.5, 2] as const;
const defaultAudioPreferences = {
  volume: 0.8,
  playbackRate: 1,
  autoPlayNext: true,
};
const sleepTimerOptions = [
  { kind: "chapters", amount: 1, label: "播放完 1 章" },
  { kind: "chapters", amount: 2, label: "播放完 2 章" },
  { kind: "chapters", amount: 3, label: "播放完 3 章" },
  { kind: "minutes", amount: 30, label: "播放 30 分钟" },
  { kind: "minutes", amount: 60, label: "播放 1 小时" },
  { kind: "minutes", amount: 120, label: "播放 2 小时" },
  { kind: "minutes", amount: 180, label: "播放 3 小时" },
] as const;
type AudioPlayerPreferences = typeof defaultAudioPreferences;
type SleepTimerOption = (typeof sleepTimerOptions)[number];
type SleepTimerState = {
  kind: SleepTimerOption["kind"];
  total: number;
  remaining: number;
};

function readAudioPreferences(): AudioPlayerPreferences {
  if (typeof window === "undefined") return { ...defaultAudioPreferences };
  try {
    const stored = window.localStorage.getItem(AUDIO_PLAYER_PREFERENCES_KEY);
    if (!stored) return { ...defaultAudioPreferences };
    const parsed = JSON.parse(stored) as Partial<AudioPlayerPreferences>;
    const rate = playbackRates.includes(
      parsed.playbackRate as (typeof playbackRates)[number],
    )
      ? parsed.playbackRate!
      : defaultAudioPreferences.playbackRate;
    return {
      volume:
        typeof parsed.volume === "number" && Number.isFinite(parsed.volume)
          ? Math.min(1, Math.max(0, parsed.volume))
          : defaultAudioPreferences.volume,
      playbackRate: rate,
      autoPlayNext:
        typeof parsed.autoPlayNext === "boolean"
          ? parsed.autoPlayNext
          : defaultAudioPreferences.autoPlayNext,
    };
  } catch {
    return { ...defaultAudioPreferences };
  }
}

const desk = requireDesk();
const router = useRouter();
const bookName = computed(
  () =>
    desk.localBook.value?.audio?.title ||
    desk.localBook.value?.name ||
    "本地有声书",
);
const author = computed(() => desk.localBook.value?.audio?.author || "");
const cover = computed(() => desk.localBook.value?.audio?.cover);
const tracks = computed(() => desk.localBook.value?.audioTracks || []);
const initialTrackIndex = computed(() => desk.localAudioTrackIndex.value);

const audio = ref<HTMLAudioElement>();
const trackIndex = ref(0);
const playing = ref(false);
const currentTime = ref(0);
const timelinePosition = ref(0);
const seeking = ref(false);
const duration = ref(0);
const savedAudioPreferences = readAudioPreferences();
const volume = ref(savedAudioPreferences.volume);
const playbackRate = ref(savedAudioPreferences.playbackRate);
const autoPlayNext = ref(savedAudioPreferences.autoPlayNext);
const timerMenuOpen = ref(false);
const desktopSettingsOpen = ref(false);
const desktopPlaylistOpen = ref(false);
const desktopSettingsButton = ref<HTMLButtonElement>();
const desktopSettingsPanel = ref<HTMLElement>();
const desktopSettingsStyle = ref<Record<string, string>>({
  visibility: "hidden",
});
const sleepTimer = ref<SleepTimerState>();
const sleepTimerSelection = ref<SleepTimerOption>(sleepTimerOptions[0]);
type MobileDrawer = "settings" | "playlist";

const mobileDrawer = ref<MobileDrawer | null>(null);
const mobileSettingsOpen = computed(() => mobileDrawer.value === "settings");
const playlistOpen = computed(() => mobileDrawer.value === "playlist");
const chapterTimerOptions = sleepTimerOptions.filter(
  (option) => option.kind === "chapters",
);
const durationTimerOptions = sleepTimerOptions.filter(
  (option) => option.kind === "minutes",
);
const compactViewport = ref(
  typeof window !== "undefined" &&
    window.matchMedia("(max-width: 800px)").matches,
);
const currentTrack = computed(() => tracks.value[trackIndex.value]);

let compactViewportQuery: MediaQueryList | undefined;
let pendingPlay = false;
let sleepTimerTimeout: number | undefined;
let sleepTimerInterval: number | undefined;

const sleepTimerLabel = computed(() => {
  const timer = sleepTimer.value;
  if (!timer) return "";
  if (timer.kind === "chapters") return `${timer.remaining} 章`;
  if (timer.remaining >= 3600) {
    const hours = Math.ceil(timer.remaining / 3600);
    return `${hours} 小时`;
  }
  return `${Math.ceil(timer.remaining / 60)} 分钟`;
});

function applyAudioPreferences() {
  if (!audio.value) return;
  audio.value.volume = volume.value;
  audio.value.playbackRate = playbackRate.value;
}

function saveAudioPreferences() {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(
      AUDIO_PLAYER_PREFERENCES_KEY,
      JSON.stringify({
        volume: volume.value,
        playbackRate: playbackRate.value,
        autoPlayNext: autoPlayNext.value,
      }),
    );
  } catch {
    // Storage can be unavailable in restricted browser contexts.
  }
}

function syncCompactViewport() {
  compactViewport.value = compactViewportQuery?.matches ?? false;
  if (compactViewport.value) {
    desktopSettingsOpen.value = false;
    desktopPlaylistOpen.value = false;
  } else {
    closeMobileDrawers();
  }
}

function positionDesktopSettings() {
  if (!desktopSettingsOpen.value || compactViewport.value) return;
  const button = desktopSettingsButton.value;
  const panel = desktopSettingsPanel.value;
  if (!button || !panel) return;

  const buttonRect = button.getBoundingClientRect();
  const panelWidth = panel.offsetWidth;
  const panelHeight = panel.offsetHeight;
  const viewportPadding = 16;
  const buttonGap = 12;
  const maxLeft = Math.max(
    viewportPadding,
    window.innerWidth - panelWidth - viewportPadding,
  );
  const left = Math.min(
    Math.max(buttonRect.right - panelWidth, viewportPadding),
    maxLeft,
  );
  const spaceBelow =
    window.innerHeight - buttonRect.bottom - buttonGap - viewportPadding;
  const spaceAbove = buttonRect.top - buttonGap - viewportPadding;
  const placeAbove = panelHeight > spaceBelow && spaceAbove > spaceBelow;
  const preferredTop = placeAbove
    ? buttonRect.top - panelHeight - buttonGap
    : buttonRect.bottom + buttonGap;
  const maxTop = Math.max(
    viewportPadding,
    window.innerHeight - panelHeight - viewportPadding,
  );
  const top = Math.min(Math.max(preferredTop, viewportPadding), maxTop);

  desktopSettingsStyle.value = {
    top: `${Math.round(top)}px`,
    left: `${Math.round(left)}px`,
    visibility: "visible",
  };
}

function handleDesktopSettingsViewportChange() {
  if (!desktopSettingsOpen.value) return;
  void nextTick(positionDesktopSettings);
}

onMounted(() => {
  compactViewportQuery = window.matchMedia("(max-width: 800px)");
  syncCompactViewport();
  compactViewportQuery.addEventListener("change", syncCompactViewport);
  window.addEventListener("resize", handleDesktopSettingsViewportChange);
  window.addEventListener(
    "scroll",
    handleDesktopSettingsViewportChange,
    true,
  );
  if (!desk.localBook.value?.audioTracks.length) {
    void router.replace("/library");
    return;
  }
  applyAudioPreferences();
  desk.navigate("audioPlayer");
});

onBeforeUnmount(() => {
  compactViewportQuery?.removeEventListener("change", syncCompactViewport);
  window.removeEventListener("resize", handleDesktopSettingsViewportChange);
  window.removeEventListener(
    "scroll",
    handleDesktopSettingsViewportChange,
    true,
  );
  clearSleepTimer();
});

function closeMobileDrawers() {
  mobileDrawer.value = null;
}

function toggleMobileSettings() {
  timerMenuOpen.value = false;
  mobileDrawer.value = mobileSettingsOpen.value ? null : "settings";
}

function openMobilePlaylist() {
  timerMenuOpen.value = false;
  mobileDrawer.value = "playlist";
}

function toggleDesktopSettings() {
  if (compactViewport.value) return;
  timerMenuOpen.value = false;
  desktopSettingsOpen.value = !desktopSettingsOpen.value;
  if (desktopSettingsOpen.value) {
    desktopSettingsStyle.value = { visibility: "hidden" };
    void nextTick(positionDesktopSettings);
  }
}

function toggleDesktopPlaylist() {
  if (compactViewport.value) return;
  desktopSettingsOpen.value = false;
  desktopPlaylistOpen.value = !desktopPlaylistOpen.value;
}

function togglePlaylist() {
  if (compactViewport.value) {
    if (playlistOpen.value) closeMobileDrawers();
    else openMobilePlaylist();
    return;
  }
  toggleDesktopPlaylist();
}

function closePlaylistDrawer() {
  if (compactViewport.value) closeMobileDrawers();
  else desktopPlaylistOpen.value = false;
}

function toggleAutoPlay() {
  autoPlayNext.value = !autoPlayNext.value;
}

function formatTimerRemaining(seconds: number) {
  if (seconds >= 3600) {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return minutes ? `${hours} 小时 ${minutes} 分钟` : `${hours} 小时`;
  }
  return `${Math.ceil(seconds / 60)} 分钟`;
}

function clearSleepTimer() {
  if (sleepTimerTimeout != null) {
    window.clearTimeout(sleepTimerTimeout);
    sleepTimerTimeout = undefined;
  }
  if (sleepTimerInterval != null) {
    window.clearInterval(sleepTimerInterval);
    sleepTimerInterval = undefined;
  }
  sleepTimer.value = undefined;
}

function stopForSleepTimer() {
  clearSleepTimer();
  pause();
}

function setSleepTimer(option: SleepTimerOption) {
  clearSleepTimer();
  sleepTimerSelection.value = option;
  sleepTimer.value = {
    kind: option.kind,
    total: option.amount,
    remaining: option.kind === "chapters" ? option.amount : option.amount * 60,
  };
  if (option.kind === "minutes") {
    sleepTimerTimeout = window.setTimeout(
      stopForSleepTimer,
      option.amount * 60 * 1000,
    );
    sleepTimerInterval = window.setInterval(() => {
      if (!sleepTimer.value) return;
      sleepTimer.value.remaining = Math.max(0, sleepTimer.value.remaining - 1);
    }, 1000);
  }
}

function toggleSleepTimerMenu() {
  mobileDrawer.value = null;
  desktopSettingsOpen.value = false;
  timerMenuOpen.value = !timerMenuOpen.value;
}

function isSleepTimerOptionSelected(option: SleepTimerOption) {
  return (
    sleepTimerSelection.value.kind === option.kind &&
    sleepTimerSelection.value.amount === option.amount
  );
}

function selectSleepTimerOption(option: SleepTimerOption) {
  sleepTimerSelection.value = option;
  if (sleepTimer.value) setSleepTimer(option);
}

function toggleSleepTimer() {
  if (sleepTimer.value) {
    clearSleepTimer();
  } else {
    setSleepTimer(sleepTimerSelection.value);
  }
}

const formatTime = (seconds: number) => {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${String(Math.floor(seconds % 60)).padStart(2, "0")}`;
};

async function play() {
  if (!audio.value) return;
  applyAudioPreferences();
  try {
    await audio.value.play();
    playing.value = true;
  } catch {
    playing.value = false;
  }
}

function pause() {
  pendingPlay = false;
  audio.value?.pause();
  playing.value = false;
}

function togglePlayback() {
  if (playing.value) pause();
  else void play();
}

function selectTrack(index: number, shouldPlay = playing.value) {
  if (!tracks.value.length) return;
  trackIndex.value = Math.min(Math.max(0, index), tracks.value.length - 1);
  currentTime.value = 0;
  timelinePosition.value = 0;
  seeking.value = false;
  duration.value = 0;
  pendingPlay = shouldPlay;
  void nextTick(() => {
    applyAudioPreferences();
    audio.value?.load();
  });
}

function previous() {
  selectTrack(
    trackIndex.value > 0 ? trackIndex.value - 1 : tracks.value.length - 1,
  );
}

function next() {
  selectTrack(
    trackIndex.value < tracks.value.length - 1 ? trackIndex.value + 1 : 0,
  );
}

function handleEnded() {
  playing.value = false;
  const timer = sleepTimer.value;
  if (timer?.kind === "chapters") {
    if (timer.remaining <= 1 || trackIndex.value >= tracks.value.length - 1) {
      clearSleepTimer();
      return;
    }
    timer.remaining -= 1;
    selectTrack(trackIndex.value + 1, true);
    return;
  }
  if (!autoPlayNext.value || trackIndex.value >= tracks.value.length - 1) {
    return;
  }
  selectTrack(trackIndex.value + 1, true);
}

function updateTimeline(event: Event) {
  const value = Number((event.target as HTMLInputElement).value);
  if (!Number.isFinite(value)) return;
  timelinePosition.value = value;
  currentTime.value = value;
  if (audio.value) audio.value.currentTime = value;
}

function startSeeking() {
  seeking.value = true;
  timelinePosition.value = currentTime.value;
}

function finishSeeking() {
  if (!seeking.value) return;
  if (audio.value) audio.value.currentTime = timelinePosition.value;
  currentTime.value = timelinePosition.value;
  seeking.value = false;
}

function syncPlaybackTime() {
  if (!audio.value || seeking.value) return;
  currentTime.value = audio.value.currentTime;
  timelinePosition.value = audio.value.currentTime;
}

function handleLoadedMetadata() {
  applyAudioPreferences();
  duration.value = audio.value?.duration || 0;
  syncPlaybackTime();
  if (pendingPlay) {
    pendingPlay = false;
    void play();
  }
}

function changeVolume(event: Event) {
  volume.value = Number((event.target as HTMLInputElement).value);
  if (audio.value) audio.value.volume = volume.value;
}

function setPlaybackRate(rate: number) {
  playbackRate.value = rate;
}
watch([volume, playbackRate, autoPlayNext], () => {
  applyAudioPreferences();
  saveAudioPreferences();
});
watch(
  () => initialTrackIndex.value,
  (index) => {
    if (!tracks.value.length) return;
    selectTrack(index || 0, false);
  },
  { immediate: true },
);
</script>

<template>
  <section class="audio-player-page">
    <div class="audio-backdrop" aria-hidden="true">
      <img v-if="cover" :src="cover" alt="" />
    </div>
    <button
      class="back-button"
      @click="router.push(`/library/${encodeURIComponent(bookName)}`)"
    >
      <ChevronLeft :size="17" />
    </button>
    <Transition name="drawer-backdrop">
      <div
        v-if="desktopSettingsOpen && !compactViewport"
        class="desktop-settings-backdrop"
        aria-hidden="true"
        @click="desktopSettingsOpen = false"
      />
    </Transition>
    <section class="music-player">
      <div class="player-main">
        <div class="cover-frame" :class="{ playing }">
          <img v-if="cover" :src="cover" :alt="`${bookName} 封面`" />
          <Headphones v-else :size="66" />
        </div>
        <div class="book-meta">
          <h2>{{ bookName }}</h2>
          <span>{{ author }}</span>
        </div>
        <div class="now-playing">
          <p>正在播放</p>
          <h3>{{ currentTrack?.title || "没有可播放的章节" }}</h3>
          <span>第 {{ trackIndex + 1 }} 章，共 {{ tracks.length }} 章</span>
        </div>
        <audio
          ref="audio"
          :src="currentTrack?.href"
          preload="metadata"
          @loadedmetadata="handleLoadedMetadata"
          @timeupdate="syncPlaybackTime"
          @play="playing = true"
          @pause="playing = false"
          @ended="handleEnded"
        />
        <div class="timeline">
          <input
            type="range"
            min="0"
            :max="duration || 0"
            step="0.1"
            :value="timelinePosition"
            aria-label="播放进度"
            @pointerdown="startSeeking"
            @pointerup="finishSeeking"
            @pointercancel="finishSeeking"
            @input="updateTimeline"
            @change="finishSeeking"
          />
          <div>
            <span>{{ formatTime(currentTime) }}</span
            ><span>{{ formatTime(duration) }}</span>
          </div>
        </div>
        <div class="control-row">
          <div class="control-cluster control-cluster-start">
            <div class="desktop-timer-action">
              <button
                class="icon-button timer-button"
                :class="{ active: sleepTimer }"
                :aria-expanded="timerMenuOpen"
                aria-haspopup="dialog"
                :aria-label="
                  sleepTimer ? '播放定时：' + sleepTimerLabel : '播放定时'
                "
                :title="
                  sleepTimer ? '播放定时：' + sleepTimerLabel : '播放定时'
                "
                @click="toggleSleepTimerMenu"
              >
                <TimerOff v-if="sleepTimer" :size="18" />
                <Timer v-else :size="18" />
                <small v-if="sleepTimer" class="timer-badge">{{
                  sleepTimerLabel
                }}</small>
              </button>
            </div>
            <div class="mobile-settings-action">
              <button
                class="icon-button"
                :class="{ active: mobileSettingsOpen }"
                :aria-expanded="mobileSettingsOpen"
                title="播放设置"
                @click="toggleMobileSettings"
              >
                <SlidersHorizontal :size="19" />
              </button>
            </div>
          </div>
          <div class="playback-controls">
            <button title="上一章" :disabled="!tracks.length" @click="previous">
              <SkipBack :size="21" /></button
            ><button
              class="play-button"
              :title="playing ? '暂停' : '播放'"
              :disabled="!tracks.length"
              @click="togglePlayback"
            >
              <Pause v-if="playing" :size="27" /><Play
                v-else
                :size="27"
                fill="currentColor"
              /></button
            ><button title="下一章" :disabled="!tracks.length" @click="next">
              <SkipForward :size="21" />
            </button>
          </div>
          <div class="control-cluster control-cluster-end">
            <div class="desktop-settings-action">
              <button
                ref="desktopSettingsButton"
                class="icon-button"
                :class="{ active: desktopSettingsOpen }"
                :aria-expanded="desktopSettingsOpen"
                aria-haspopup="dialog"
                title="播放设置"
                @click="toggleDesktopSettings"
              >
                <SlidersHorizontal :size="19" />
              </button>
            </div>
            <div class="desktop-playlist-action">
              <button
                class="icon-button"
                :class="{ active: desktopPlaylistOpen }"
                :aria-expanded="desktopPlaylistOpen"
                :aria-label="
                  desktopPlaylistOpen ? '隐藏章节列表' : '显示章节列表'
                "
                :title="desktopPlaylistOpen ? '隐藏章节列表' : '显示章节列表'"
                @click="toggleDesktopPlaylist"
              >
                <ListMusic :size="20" />
              </button>
            </div>
            <div class="mobile-playlist-action">
              <button
                class="icon-button"
                :aria-expanded="playlistOpen"
                title="章节目录"
                @click="openMobilePlaylist"
              >
                <ListMusic :size="20" />
              </button>
            </div>
          </div>
        </div>
      </div>
    </section>
    <Transition name="drawer-backdrop">
      <div
        v-if="timerMenuOpen"
        class="timer-menu-backdrop"
        aria-hidden="true"
        @click="timerMenuOpen = false"
      />
    </Transition>
    <Transition name="timer-popover">
      <section
        v-if="timerMenuOpen"
        class="timer-menu"
        role="dialog"
        aria-modal="true"
        aria-labelledby="timer-menu-title"
        @click.stop
      >
      <header>
        <div>
          <small>PLAYBACK TIMER</small>
          <h3 id="timer-menu-title">播放定时</h3>
        </div>
        <button title="关闭播放定时" @click="timerMenuOpen = false">
          <X :size="18" />
        </button>
      </header>
      <div class="timer-toggle-row">
        <div>
          <strong>定时播放</strong>
          <small>{{
            sleepTimer ? `已开启 · ${sleepTimerLabel}后停止` : "开启后自动停止"
          }}</small>
        </div>
        <button
          class="timer-switch"
          :class="{ active: sleepTimer }"
          role="switch"
          :aria-checked="Boolean(sleepTimer)"
          :aria-label="sleepTimer ? '关闭定时播放' : '开启定时播放'"
          @click="toggleSleepTimer"
        >
          <span />
        </button>
      </div>
      <div class="timer-groups">
        <section class="timer-group">
          <h4>按集数</h4>
          <div class="timer-options">
            <button
              v-for="option in chapterTimerOptions"
              :key="option.kind + '-' + option.amount"
              :class="{ active: isSleepTimerOptionSelected(option) }"
              :aria-pressed="isSleepTimerOptionSelected(option)"
              @click="selectSleepTimerOption(option)"
            >
              <span>{{ option.label }}</span>
              <Check v-if="isSleepTimerOptionSelected(option)" :size="17" />
            </button>
          </div>
        </section>
        <section class="timer-group">
          <h4>按时间</h4>
          <div class="timer-options">
            <button
              v-for="option in durationTimerOptions"
              :key="option.kind + '-' + option.amount"
              :class="{ active: isSleepTimerOptionSelected(option) }"
              :aria-pressed="isSleepTimerOptionSelected(option)"
              @click="selectSleepTimerOption(option)"
            >
              <span>{{ option.label }}</span>
              <Check v-if="isSleepTimerOptionSelected(option)" :size="17" />
            </button>
          </div>
        </section>
      </div>
      <p v-if="sleepTimer" class="timer-status">
        将在<span v-if="sleepTimer.kind === 'chapters'"
          >播放完 {{ sleepTimer.remaining }} 章后</span
        ><span v-else>{{ formatTimerRemaining(sleepTimer.remaining) }}</span
        >停止
      </p>
      <p v-else class="timer-status">
        当前选择：{{ sleepTimerSelection.label }}
      </p>
      </section>
    </Transition>
    <Transition name="settings-popover">
      <section
        v-if="desktopSettingsOpen && !compactViewport"
        ref="desktopSettingsPanel"
        class="desktop-player-settings"
        :style="desktopSettingsStyle"
        role="dialog"
        aria-modal="true"
        aria-labelledby="desktop-player-settings-title"
        @click.stop
      >
      <header>
        <div>
          <small>PLAYER SETTINGS</small>
          <h3 id="desktop-player-settings-title">播放设置</h3>
        </div>
        <button title="关闭播放设置" @click="desktopSettingsOpen = false">
          <X :size="18" />
        </button>
      </header>
      <label class="volume-setting">
        <Volume2 :size="18" />
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          :value="volume"
          aria-label="音量"
          @input="changeVolume"
        />
        <span>{{ Math.round(volume * 100) }}%</span>
      </label>
      <div class="playback-setting-row">
        <div>
          <strong>自动连播</strong>
          <small>播放完当前章节后继续下一章</small>
        </div>
        <button
          class="settings-switch"
          :class="{ active: autoPlayNext }"
          role="switch"
          :aria-checked="autoPlayNext"
          :aria-label="autoPlayNext ? '关闭自动连播' : '开启自动连播'"
          @click="toggleAutoPlay"
        >
          <span />
        </button>
      </div>
      <div class="desktop-rate-setting">
        <span>播放速度</span>
        <div class="rate-options" aria-label="播放速度">
          <button
            v-for="rate in playbackRates"
            :key="rate"
            :class="{ active: playbackRate === rate }"
            @click="setPlaybackRate(rate)"
          >
            {{ rate }}x
          </button>
        </div>
      </div>
      </section>
    </Transition>
    <AudioPlayerDrawer
      :open="mobileSettingsOpen"
      @close="closeMobileDrawers"
    >
      <section class="player-drawer mobile-player-settings">
        <div class="drawer-heading">
          <strong>播放设置</strong>
          <button title="关闭播放设置" @click="closeMobileDrawers">
            <X :size="19" />
          </button>
        </div>
        <label class="volume-setting">
          <Volume2 :size="18" />
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            :value="volume"
            aria-label="音量"
            @input="changeVolume"
          />
          <span>{{ Math.round(volume * 100) }}%</span>
        </label>
        <div class="mobile-rate-options" aria-label="播放速度">
          <button
            v-for="rate in playbackRates"
            :key="rate"
            :class="{ active: playbackRate === rate }"
            @click="setPlaybackRate(rate)"
          >
            {{ rate }}x
          </button>
        </div>
        <div class="playback-setting-row">
          <div>
            <strong>自动连播</strong>
            <small>播放完当前章节后继续下一章</small>
          </div>
          <button
            class="settings-switch"
            :class="{ active: autoPlayNext }"
            role="switch"
            :aria-checked="autoPlayNext"
            :aria-label="autoPlayNext ? '关闭自动连播' : '开启自动连播'"
            @click="toggleAutoPlay"
          >
            <span />
          </button>
        </div>
        <button class="mobile-timer-entry" @click="toggleSleepTimerMenu">
          <span>
            <strong>播放定时</strong>
            <small>{{
              sleepTimer ? `已开启 · ${sleepTimerLabel}` : "未开启"
            }}</small>
          </span>
          <TimerOff v-if="sleepTimer" :size="18" /><Timer v-else :size="18" />
          <ChevronRight :size="17" />
        </button>
      </section>
    </AudioPlayerDrawer>
    <AudioPlayerDrawer
      :open="playlistOpen || desktopPlaylistOpen"
      :backdrop-class="
        compactViewport ? 'playlist-backdrop' : 'desktop-playlist-backdrop'
      "
      @close="closePlaylistDrawer"
    >
      <aside
        class="player-drawer track-section"
        :aria-hidden="compactViewport ? !playlistOpen : !desktopPlaylistOpen"
      >
        <button
          class="track-heading"
          :aria-expanded="compactViewport ? playlistOpen : desktopPlaylistOpen"
          @click="togglePlaylist"
        >
          <span><ListMusic :size="19" />章节目录</span
          ><small>{{ tracks.length }} 章</small
          ><X class="drawer-close" :size="19" />
        </button>
        <div class="track-list">
          <button
            v-for="(track, index) in tracks"
            :key="track.href"
            :class="{ active: index === trackIndex }"
            @click="selectTrack(index, true)"
          >
            <span>{{ String(index + 1).padStart(3, "0") }}</span
            ><strong>{{ track.title }}</strong
            ><Play v-if="index === trackIndex && !playing" :size="16" /><Pause
              v-else-if="index === trackIndex"
              :size="16"
            />
          </button>
        </div>
      </aside>
    </AudioPlayerDrawer>
  </section>
</template>

<style scoped>
/* Shared layout and playback controls */
.audio-player-page {
  box-sizing: border-box;
  position: relative;
  display: flex;
  height: 100dvh;
  min-height: 0;
  flex-direction: column;
  padding: 5px 8px 10px;
  overflow: hidden;
  isolation: isolate;
  background: #5f4a46;
}
.audio-backdrop {
  position: absolute;
  z-index: 0;
  inset: -32px;
  overflow: hidden;
  background: linear-gradient(145deg, #8e6258, #413944);
}
.audio-backdrop img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  filter: blur(24px);
  opacity: 0.88;
  transform: scale(1.08);
}
.back-button {
  position: relative;
  z-index: 1;
  display: inline-flex;
  align-items: center;
  gap: 3px;
  border: 0;
  font-size: 12px;
  font-weight: 600;
}
.music-player {
  position: relative;
  z-index: 1;
  display: flex;
  height: auto;
  flex: 1 1 auto;
  min-height: 0;
  align-items: center;
  justify-content: center;
  overflow: visible;
}
.player-main {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  align-items: center;
  padding: clamp(18px, 3vh, 34px) clamp(24px, 6vw, 78px) 18px;
}
.cover-frame {
  display: grid;
  width: min(275px, 28vh);
  aspect-ratio: 1;
  overflow: hidden;
  flex: 0 1 auto;
  border: 9px solid rgba(255, 255, 255, 0.84);
  border-radius: 8px;
  color: #a6535c;
  background: #fdf1f0;
  box-shadow: 0 18px 34px rgba(129, 74, 57, 0.24);
  place-items: center;
}
.cover-frame.playing {
  animation: soft-pulse 2.8s ease-in-out infinite;
}
.cover-frame img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.book-meta {
  width: 100%;
  margin: 14px 0 0;
  text-align: center;
}
.book-meta p,
.now-playing p {
  margin: 0 0 4px;
  color: #b08072;
  font-size: 11px;
}
.book-meta h2 {
  overflow: hidden;
  margin: 0;
  color: #69453d;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 25px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.book-meta span,
.now-playing span {
  color: #a47b6d;
  font-size: 12px;
}
.now-playing {
  width: 100%;
  min-width: 0;
  margin-top: 20px;
}
.now-playing h3 {
  overflow: hidden;
  margin: 0 0 3px;
  color: #754e44;
  font-size: 16px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.timeline {
  width: 100%;
  margin-top: 14px;
}
.timeline input {
  width: 100%;
  height: 5px;
  margin: 0;
  accent-color: var(--theme-color);
  cursor: pointer;
}
.timeline div {
  display: flex;
  justify-content: space-between;
  margin-top: 4px;
  color: #a47b6d;
  font: 11px monospace;
}
.control-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  column-gap: 12px;
  align-items: center;
  width: 100%;
  margin-top: 9px;
}
.control-cluster {
  display: flex;
  min-width: 0;
  width: 100%;
  align-items: center;
  gap: 8px;
}
.control-cluster-start {
  justify-self: start;
}
.control-cluster-end {
  justify-content: flex-end;
  justify-self: end;
  gap: 12px;
}
.icon-button {
  position: relative;
  display: grid;
  width: 38px;
  height: 38px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border: 1px solid rgba(187, 132, 111, 0.2);
  border-radius: 50%;
  color: #895c51;
  background: rgba(255, 242, 234, 0.72);
  place-items: center;
}
.icon-button:hover,
.icon-button.active {
  border-color: var(--theme-color);
  color: var(--theme-color-dark);
  background: rgba(255, 244, 237, 0.96);
}
.icon-button.active {
  box-shadow: 0 0 0 3px rgba(226, 148, 100, 0.13);
}
.timer-badge {
  position: absolute;
  right: -5px;
  bottom: -5px;
  max-width: 42px;
  overflow: hidden;
  padding: 2px 4px;
  border: 1px solid rgba(255, 255, 255, 0.82);
  border-radius: 5px;
  color: #fff;
  background: var(--theme-color);
  font-size: 8px;
  line-height: 1.1;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.playback-controls {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 15px;
}
.playback-controls button {
  display: grid;
  width: 40px;
  height: 40px;
  border: 0;
  border-radius: 50%;
  color: #895c51;
  background: rgba(255, 242, 234, 0.9);
  place-items: center;
}
.playback-controls .play-button {
  width: 55px;
  height: 55px;
  color: #fff;
  background: var(--theme-color);
  box-shadow: 0 8px 17px #d97b5450;
}
.playback-controls button:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
.volume-control {
  display: flex;
  align-items: center;
  gap: 7px;
  color: #a17367;
}
.volume-control input {
  width: 78px;
  accent-color: var(--theme-color);
}
.rate-options {
  display: flex;
  gap: 3px;
}
.rate-options button {
  min-width: 35px;
  padding: 4px 3px;
  border: 0;
  border-radius: 4px;
  color: #895c51;
  background: transparent;
  font-size: 10px;
}
.rate-options button.active {
  color: #fff;
  background: var(--theme-color);
}
.track-section {
  display: flex;
  min-height: 0;
  flex-direction: column;
}
.track-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 0 20px 14px;
  border: 0;
  color: #69453d;
  background: none;
  font: inherit;
  text-align: left;
}
.track-heading span {
  display: flex;
  align-items: center;
  gap: 6px;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 18px;
}
.track-heading small {
  color: #ad877a;
  font-size: 12px;
}
.track-list {
  min-height: 0;
  flex: 1 1 auto;
  overflow-y: auto;
  border-top: 1px solid rgba(187, 132, 111, 0.15);
}
.track-list button {
  display: grid;
  grid-template-columns: 37px minmax(0, 1fr) 20px;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-height: 54px;
  padding: 0 18px;
  border: 0;
  border-bottom: 1px solid rgba(187, 132, 111, 0.12);
  color: #65433c;
  background: none;
  text-align: left;
}
.track-list button:hover,
.track-list button.active {
  background: #fff1e9;
}
.track-list button.active {
  color: var(--theme-color-dark);
}
.track-list span {
  color: #a47b6d;
  font: 11px monospace;
}
.track-list strong {
  overflow: hidden;
  font-size: 13px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.desktop-timer-action,
.desktop-settings-action,
.desktop-playlist-action,
.mobile-settings-action,
.mobile-playlist-action {
  display: none;
}
.timer-menu-backdrop {
  position: fixed;
  z-index: 17;
  inset: 0;
  background: transparent;
}
.timer-menu {
  position: fixed;
  z-index: 18;
  top: 50%;
  left: 50%;
  width: min(390px, calc(100vw - 32px));
  padding: 17px 17px 14px;
  border: 1px solid rgba(224, 191, 174, 0.8);
  border-radius: 18px;
  background: rgba(255, 251, 248, 0.96);
  box-shadow: 0 18px 46px rgba(75, 44, 37, 0.2);
  transform: translate(-50%, -50%);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
}
.timer-popover-enter-active,
.timer-popover-leave-active,
.settings-popover-enter-active,
.settings-popover-leave-active {
  transition:
    transform 0.24s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.2s ease;
  will-change: transform, opacity;
}
.timer-popover-enter-from,
.timer-popover-leave-to {
  opacity: 0;
  transform: translate(-50%, -50%) scale(0.96);
}
.timer-popover-enter-to,
.timer-popover-leave-from {
  opacity: 1;
  transform: translate(-50%, -50%) scale(1);
}
.settings-popover-enter-from,
.settings-popover-leave-to {
  opacity: 0;
  transform: translateY(14px) scale(0.96);
}
.settings-popover-enter-to,
.settings-popover-leave-from {
  opacity: 1;
  transform: translateY(0) scale(1);
}
.timer-menu header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: 12px;
  border-bottom: 1px solid rgba(187, 132, 111, 0.15);
}
.timer-menu header small {
  color: #b59885;
  font: 9px monospace;
  letter-spacing: 0.08em;
}
.timer-menu h3 {
  margin: 4px 0 0;
  color: #57423b;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 18px;
}
.timer-menu header button {
  display: grid;
  width: 31px;
  height: 31px;
  border: 0;
  border-radius: 50%;
  color: #895c51;
  background: #f7ebe4;
  place-items: center;
}
.timer-toggle-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  padding: 14px 0 11px;
  border-bottom: 1px solid rgba(187, 132, 111, 0.15);
}
.timer-toggle-row > div {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 3px;
}
.timer-toggle-row strong {
  color: #684a41;
  font-size: 13px;
}
.timer-toggle-row small {
  overflow: hidden;
  color: #ad877a;
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.timer-switch {
  position: relative;
  display: inline-flex;
  width: 45px;
  height: 26px;
  flex: 0 0 auto;
  align-items: center;
  padding: 3px;
  border: 0;
  border-radius: 999px;
  background: #d8c2b8;
  transition: background 0.18s ease;
}
.timer-switch span {
  display: block;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: #fffaf7;
  box-shadow: 0 2px 5px rgba(75, 44, 37, 0.2);
  transition: transform 0.18s ease;
}
.timer-switch.active {
  background: var(--theme-color);
}
.timer-switch.active span {
  transform: translateX(19px);
}
.timer-groups {
  display: grid;
  gap: 14px;
  padding-top: 13px;
}
.timer-group h4 {
  margin: 0;
  color: #a26658;
  font-size: 11px;
  font-weight: 700;
}
.timer-group .timer-options {
  padding-top: 7px;
}
.timer-options {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 7px;
  padding-top: 13px;
}
.timer-options button {
  display: flex;
  min-height: 38px;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  padding: 0 9px;
  border: 1px solid #ead8cc;
  border-radius: 7px;
  color: #795e53;
  background: rgba(255, 253, 250, 0.8);
  font-size: 11px;
  text-align: left;
}
.timer-options button:hover,
.timer-options button.active {
  border-color: var(--theme-color);
  color: var(--theme-color-dark);
  background: #fff1e8;
}
.timer-options button svg {
  flex: 0 0 auto;
}
.timer-status {
  margin: 8px 0 0;
  color: #ad877a;
  font-size: 10px;
  text-align: right;
}
.desktop-settings-backdrop {
  position: fixed;
  z-index: 17;
  inset: 0;
  background: transparent;
}
.desktop-player-settings {
  box-sizing: border-box;
  position: fixed;
  z-index: 18;
  width: min(300px, calc(100vw - 32px));
  padding: 17px;
  border: 1px solid rgba(224, 191, 174, 0.8);
  border-radius: 16px;
  background: rgba(255, 251, 248, 0.97);
  box-shadow: 0 18px 46px rgba(75, 44, 37, 0.2);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
}
.desktop-player-settings header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: 12px;
  border-bottom: 1px solid rgba(187, 132, 111, 0.15);
}
.desktop-player-settings header small {
  color: #b59885;
  font: 9px monospace;
  letter-spacing: 0.08em;
}
.desktop-player-settings h3 {
  margin: 4px 0 0;
  color: #57423b;
  font-family: KaTongFont, "Microsoft YaHei", sans-serif;
  font-size: 18px;
}
.desktop-player-settings header button {
  display: grid;
  width: 31px;
  height: 31px;
  border: 0;
  border-radius: 50%;
  color: #895c51;
  background: #f7ebe4;
  place-items: center;
}
.desktop-player-settings .volume-setting {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 16px;
  color: #a17367;
}
.desktop-player-settings .volume-setting input[type="range"] {
  width: 100%;
  min-width: 0;
  accent-color: var(--theme-color);
}
.desktop-player-settings .volume-setting span {
  min-width: 34px;
  color: #895c51;
  font: 11px monospace;
  text-align: right;
}
.desktop-rate-setting {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 16px;
  color: #795e53;
  font-size: 11px;
}
.playback-setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  margin-top: 16px;
  padding-top: 14px;
  border-top: 1px solid rgba(187, 132, 111, 0.15);
}
.playback-setting-row > div {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 3px;
}
.playback-setting-row strong {
  color: #684a41;
  font-size: 12px;
}
.playback-setting-row small {
  overflow: hidden;
  color: #ad877a;
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.settings-switch {
  position: relative;
  display: inline-flex;
  width: 45px;
  height: 26px;
  flex: 0 0 auto;
  align-items: center;
  padding: 3px;
  border: 0;
  border-radius: 999px;
  background: #d8c2b8;
  transition: background 0.18s ease;
}
.settings-switch span {
  display: block;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: #fffaf7;
  box-shadow: 0 2px 5px rgba(75, 44, 37, 0.2);
  transition: transform 0.18s ease;
}
.settings-switch.active {
  background: var(--theme-color);
}
.settings-switch.active span {
  transform: translateX(19px);
}
.mobile-timer-entry {
  display: none;
}
@keyframes soft-pulse {
  50% {
    box-shadow: 0 20px 39px rgba(210, 115, 78, 0.38);
    transform: translateY(-2px);
  }
}

/* Desktop layout: centered player with an optional queue panel. */
@media (min-width: 801px) {
  .control-row {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  }
  .control-cluster-start {
    grid-column: 1;
    grid-row: 1;
  }
  .playback-controls {
    grid-column: 1 / -1;
    grid-row: 1;
    justify-self: center;
  }
  .control-cluster-end {
    grid-column: 2;
    grid-row: 1;
  }
  .back-button {
    min-height: 40px;
    width: fit-content;
    margin: 0 0 12px 4px;
    padding: 0 13px 0 8px;
    border: 1px solid rgba(255, 255, 255, 0.3);
    border-radius: 11px;
    color: rgba(255, 250, 247, 0.95);
    background: rgba(57, 39, 40, 0.24);
    box-shadow: 0 8px 20px rgba(32, 22, 29, 0.12);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
  }
  .player-main {
    width: min(680px, calc(100% - 40px));
    padding-right: clamp(18px, 3vw, 48px);
    padding-left: clamp(18px, 3vw, 48px);
    border: 1px solid rgba(255, 255, 255, 0.42);
    border-radius: 24px;
    background: rgba(255, 249, 246, 0.7);
    box-shadow: 0 20px 50px rgba(34, 24, 31, 0.24);
    backdrop-filter: blur(24px) saturate(1.08);
    -webkit-backdrop-filter: blur(24px) saturate(1.08);
  }
  .mobile-player-settings,
  .mobile-settings-action,
  .mobile-playlist-action,
  .drawer-close {
    display: none;
  }
  .desktop-timer-action,
  .desktop-settings-action,
  .desktop-playlist-action {
    display: grid;
    align-items: center;
  }
  .track-heading .drawer-close {
    display: block;
    margin-left: 12px;
  }
  .track-section {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 3;
    width: min(320px, 29vw);
    overflow: hidden;
    padding: 24px 0 0;
    border: 1px solid rgba(255, 255, 255, 0.38);
    border-radius: 20px 0 0 20px;
    background: rgba(255, 250, 247, 0.62);
    box-shadow: 0 15px 36px rgba(34, 24, 31, 0.16);
    backdrop-filter: blur(22px);
    -webkit-backdrop-filter: blur(22px);
  }
}

/* Mobile layout: touch controls and bottom-sheet drawers. */
@media (max-width: 800px) {
  .audio-player-page {
    height: 100dvh;
    min-height: 0;
    max-height: 100dvh;
    padding: calc(8px + max(env(safe-area-inset-top), 24px)) 10px
      calc(10px + max(env(safe-area-inset-bottom), 24px));
  }
  .audio-backdrop {
    position: fixed;
    inset: 0;
  }
  .back-button {
    display: flex;
    height: 42px;
    width: 42px;
    justify-content: center;
    align-items: center;
    margin: 0 0 10px;
    padding: 8px 12px 8px 8px;
    border: 1px solid rgba(255, 255, 255, 0.34);
    border-radius: 50%;
    color: #fff;
    background: rgba(69, 40, 34, 0.22);
    backdrop-filter: blur(13px);
    -webkit-backdrop-filter: blur(13px);
  }
  .music-player {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    align-items: center;
    height: auto;
    min-height: 0;
    overflow: hidden;
    border: 0;
    background: none;
  }
  .player-main {
    box-sizing: border-box;
    width: min(640px, 100%);
    max-height: 100%;
    flex: 0 1 auto;
    margin: auto 0;
    overflow: hidden;
    padding-right: 10px;
    padding-left: 10px;
    /* overflow-y: auto;
    border: 1px solid rgba(255, 255, 255, 0.55);
    border-radius: 24px;
    background: rgba(255, 248, 242, 0.66);
    box-shadow: 0 18px 35px rgba(86, 47, 38, 0.2);
    backdrop-filter: blur(18px);
    -webkit-backdrop-filter: blur(18px); */
  }
  .track-section {
    width: 100%;
    max-width: none;
    max-height: min(70dvh, 510px);
    padding: 0;
  }
  .track-heading {
    min-height: 65px;
    padding: 0 18px;
    border-bottom: 1px solid rgba(187, 132, 111, 0.14);
  }
  .drawer-close {
    display: block;
    margin-left: 12px;
  }
  .track-heading small {
    margin-left: auto;
  }
  .drawer-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 2px;
    color: #69453d;
    font-size: 16px;
  }
  .drawer-heading button {
    display: grid;
    width: 34px;
    height: 34px;
    border: 0;
    border-radius: 50%;
    color: #895c51;
    background: rgba(255, 242, 234, 0.9);
    place-items: center;
  }
  .track-list {
    padding-bottom: env(safe-area-inset-bottom);
  }
  .cover-frame {
    width: min(232px, 54vw);
    border-width: 10px;
    border-radius: 16px;
    box-shadow: 0 20px 38px rgba(93, 49, 38, 0.3);
  }
  .book-meta {
    margin-top: 18px;
  }
  .book-meta p {
    color: #9a5545;
    font-weight: 600;
  }
  .book-meta h2 {
    color: #593b35;
    font-size: clamp(22px, 6vw, 28px);
  }
  .now-playing {
    margin-top: 22px;
    padding: 14px 16px;
    border: 1px solid rgba(255, 255, 255, 0.58);
    border-radius: 17px;
    background: rgba(255, 255, 255, 0.34);
    box-shadow: inset 0 1px rgba(255, 255, 255, 0.45);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
  }
  .now-playing p {
    color: #a65a49;
    font-weight: 600;
  }
  .now-playing h3 {
    color: #5f4038;
  }
  .now-playing span {
    color: #946c60;
  }
  .control-row {
    grid-template-columns: 1fr auto 1fr;
    column-gap: 0;
    min-height: 64px;
    margin-top: 14px;
  }
  .control-cluster-start,
  .control-cluster-end {
    gap: 5px;
  }
  .control-cluster-end {
    margin-left: auto;
  }
  .volume-control,
  .rate-options {
    display: none;
  }
  .playback-controls {
    grid-column: 2;
    grid-row: 1;
    gap: 12px;
  }
  .playback-controls button {
    width: 42px;
    height: 42px;
  }
  .playback-controls .play-button {
    width: 60px;
    height: 60px;
  }
  .icon-button {
    width: 36px;
    height: 36px;
  }
  .mobile-settings-action,
  .mobile-playlist-action {
    display: grid;
    align-items: center;
  }
  .mobile-settings-action {
    grid-column: 1;
    grid-row: 1;
    justify-self: start;
  }
  .mobile-playlist-action {
    grid-column: 3;
    grid-row: 1;
    justify-self: end;
  }
  .mobile-settings-action button.active {
    color: #fff;
    background: var(--theme-color);
  }
  .mobile-player-settings {
    display: flex;
    align-items: stretch;
    flex-direction: column;
    gap: 14px;
    padding: 20px 18px calc(18px + env(safe-area-inset-bottom));
  }
  .mobile-player-settings .volume-setting {
    display: flex;
    min-width: 0;
    flex: 1;
    align-items: center;
    gap: 8px;
    color: #a17367;
  }
  .mobile-player-settings .volume-setting input[type="range"] {
    width: 100%;
    min-width: 48px;
    accent-color: var(--theme-color);
  }
  .mobile-player-settings .volume-setting span {
    min-width: 30px;
    color: #895c51;
    font: 11px monospace;
    text-align: right;
  }
  .mobile-rate-options {
    display: flex;
    align-self: center;
    gap: 3px;
  }
  .mobile-rate-options button {
    min-width: 34px;
    padding: 5px 3px;
    border: 0;
    border-radius: 4px;
    color: #895c51;
    background: transparent;
    font-size: 10px;
  }
  .mobile-rate-options button.active {
    color: #fff;
    background: var(--theme-color);
  }
  .mobile-timer-entry {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 10px;
    width: 100%;
    margin-top: 0;
    padding: 14px 0 0;
    border: 0;
    border-top: 1px solid rgba(187, 132, 111, 0.15);
    color: #795e53;
    background: transparent;
    text-align: left;
  }
  .mobile-timer-entry > span {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
    gap: 3px;
  }
  .mobile-timer-entry strong {
    color: #684a41;
    font-size: 12px;
  }
  .mobile-timer-entry small {
    overflow: hidden;
    color: #ad877a;
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .timer-menu {
    top: auto;
    right: 0;
    bottom: 0;
    left: 0;
    width: min(100%, 520px);
    max-height: min(78dvh, 620px);
    overflow-y: auto;
    margin: 0 auto;
    padding: 20px 18px calc(18px + env(safe-area-inset-bottom));
    border-right: 0;
    border-bottom: 0;
    border-left: 0;
    border-radius: 22px 22px 0 0;
    transform: none;
  }
  .timer-popover-enter-from,
  .timer-popover-leave-to {
    transform: translateY(18px);
  }
  .timer-popover-enter-to,
  .timer-popover-leave-from {
    transform: translateY(0);
  }
  .timer-menu .timer-options button {
    min-height: 42px;
  }
  .desktop-player-settings,
  .desktop-settings-backdrop {
    display: none;
  }
}
</style>
