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
  ChevronLeft,
  Headphones,
  ListMusic,
  Pause,
  Play,
  SkipBack,
  SkipForward,
  SlidersHorizontal,
  Volume2,
  X,
} from "lucide-vue-next";
import { deskInjectionKey } from "../deskContext";

function requireDesk() {
  const desk = inject(deskInjectionKey);
  if (!desk) throw new Error("Desk context is unavailable");
  return desk;
}

const desk = requireDesk();
const router = useRouter();
const bookName = computed(
  () => desk.localBook.value?.audio?.title || desk.localBook.value?.name || "本地有声书",
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
const volume = ref(0.8);
const playbackRate = ref(1);
type MobileDrawer = "settings" | "playlist";

const mobileDrawer = ref<MobileDrawer | null>(null);
const mobileSettingsOpen = computed(() => mobileDrawer.value === "settings");
const playlistOpen = computed(() => mobileDrawer.value === "playlist");
const compactViewport = ref(
  typeof window !== "undefined" &&
    window.matchMedia("(max-width: 800px)").matches,
);
const currentTrack = computed(() => tracks.value[trackIndex.value]);

let compactViewportQuery: MediaQueryList | undefined;

function syncCompactViewport() {
  compactViewport.value = compactViewportQuery?.matches ?? false;
  if (!compactViewport.value) closeMobileDrawers();
}

onMounted(() => {
  compactViewportQuery = window.matchMedia("(max-width: 800px)");
  syncCompactViewport();
  compactViewportQuery.addEventListener("change", syncCompactViewport);
  if (!desk.localBook.value?.audioTracks.length) {
    void router.replace("/library");
    return;
  }
  desk.navigate("audioPlayer");
});

onBeforeUnmount(() => {
  compactViewportQuery?.removeEventListener("change", syncCompactViewport);
});

function closeMobileDrawers() {
  mobileDrawer.value = null;
}

function toggleMobileSettings() {
  mobileDrawer.value = mobileSettingsOpen.value ? null : "settings";
}

function openMobilePlaylist() {
  mobileDrawer.value = "playlist";
}

const formatTime = (seconds: number) => {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${String(Math.floor(seconds % 60)).padStart(2, "0")}`;
};

async function play() {
  if (!audio.value) return;
  try {
    await audio.value.play();
    playing.value = true;
  } catch {
    playing.value = false;
  }
}

function pause() {
  audio.value?.pause();
  playing.value = false;
}

function togglePlayback() {
  if (playing.value) pause();
  else void play();
}

function selectTrack(index: number, shouldPlay = playing.value) {
  trackIndex.value = Math.min(Math.max(0, index), tracks.value.length - 1);
  currentTime.value = 0;
  timelinePosition.value = 0;
  seeking.value = false;
  duration.value = 0;
  void nextTick(() => {
    if (shouldPlay) void play();
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

function updateDuration() {
  duration.value = audio.value?.duration || 0;
  syncPlaybackTime();
}

function changeVolume(event: Event) {
  volume.value = Number((event.target as HTMLInputElement).value);
  if (audio.value) audio.value.volume = volume.value;
}

function setPlaybackRate(rate: number) {
  playbackRate.value = rate;
  if (audio.value) audio.value.playbackRate = rate;
}
watch(volume, (value) => {
  if (audio.value) audio.value.volume = value;
});
watch(playbackRate, (value) => {
  if (audio.value) audio.value.playbackRate = value;
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
    <div
      v-if="mobileSettingsOpen || playlistOpen"
      class="playlist-backdrop"
      aria-hidden="true"
      @click="closeMobileDrawers"
    />
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
          @loadedmetadata="updateDuration"
          @timeupdate="syncPlaybackTime"
          @play="playing = true"
          @pause="playing = false"
          @ended="next"
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
          <div class="player-options">
            <label title="音量"
              ><Volume2 :size="17" /><input
                type="range"
                min="0"
                max="1"
                step="0.05"
                :value="volume"
                aria-label="音量"
                @input="changeVolume"
            /></label>
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
          <div class="rate-options" aria-label="播放速度">
            <button
              v-for="rate in [1, 1.25, 1.5, 2]"
              :key="rate"
              :class="{ active: playbackRate === rate }"
              @click="setPlaybackRate(rate)"
            >
              {{ rate }}x
            </button>
          </div>
          <div class="mobile-settings-action">
            <button
              :class="{ active: mobileSettingsOpen }"
              :aria-expanded="mobileSettingsOpen"
              title="播放设置"
              @click="toggleMobileSettings"
            >
              <SlidersHorizontal :size="19" />
            </button>
          </div>
          <div class="mobile-playlist-action">
            <button
              :aria-expanded="playlistOpen"
              title="播放队列"
              @click="openMobilePlaylist"
            >
              <ListMusic :size="20" />
            </button>
          </div>
        </div>
      </div>
    </section>
    <section v-if="mobileSettingsOpen" class="mobile-player-settings">
      <div class="drawer-heading">
        <strong>播放设置</strong>
        <button title="关闭播放设置" @click="closeMobileDrawers">
          <X :size="19" />
        </button>
      </div>
      <label>
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
          v-for="rate in [1, 1.25, 1.5, 2]"
          :key="rate"
          :class="{ active: playbackRate === rate }"
          @click="setPlaybackRate(rate)"
        >
          {{ rate }}x
        </button>
      </div>
    </section>
    <aside
      v-show="playlistOpen || !compactViewport"
      class="track-section"
      :class="{ 'queue-open': playlistOpen }"
      :aria-hidden="compactViewport && !playlistOpen"
    >
      <button
        class="track-heading"
        :aria-expanded="playlistOpen"
        @click="playlistOpen ? closeMobileDrawers() : openMobilePlaylist()"
      >
        <span><ListMusic :size="19" />播放队列</span
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
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  width: 100%;
  margin-top: 9px;
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
.player-options {
  justify-self: start;
}
.player-options label {
  display: flex;
  align-items: center;
  gap: 7px;
  color: #a17367;
}
.player-options input {
  width: 78px;
  accent-color: var(--theme-color);
}
.rate-options {
  display: flex;
  justify-self: end;
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
@keyframes soft-pulse {
  50% {
    box-shadow: 0 20px 39px rgba(210, 115, 78, 0.38);
    transform: translateY(-2px);
  }
}

/* Desktop layout: centered player with a persistent queue panel. */
@media (min-width: 801px) {
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
    width: min(620px, calc(100% - 380px));
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
  .playlist-backdrop,
  .drawer-close {
    display: none;
  }
  .track-section {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 1;
    width: min(320px, 29vw);
    overflow: hidden;
    padding: 24px 0 0;
    border: 1px solid rgba(255, 255, 255, 0.38);
    border-radius: 20px;
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
    /* overflow-y: auto;
    border: 1px solid rgba(255, 255, 255, 0.55);
    border-radius: 24px;
    background: rgba(255, 248, 242, 0.66);
    box-shadow: 0 18px 35px rgba(86, 47, 38, 0.2);
    backdrop-filter: blur(18px);
    -webkit-backdrop-filter: blur(18px); */
  }
  .track-section {
    position: fixed;
    top: auto;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 16;
    width: 100%;
    max-width: none;
    height: auto;
    max-height: min(70dvh, 510px);
    min-height: 0;
    padding: 0;
    border: 1px solid rgba(255, 255, 255, 0.62);
    border-bottom: 0;
    border-radius: 22px 22px 0 0;
    background: rgba(255, 250, 247, 0.88);
    box-shadow: 0 -12px 32px rgba(91, 54, 44, 0.16);
    backdrop-filter: blur(18px);
    -webkit-backdrop-filter: blur(18px);
    pointer-events: none;
    transform: translateY(100%);
    transition: transform 0.24s ease;
    will-change: transform;
  }
  .track-section.queue-open {
    pointer-events: auto;
    transform: translateY(0);
  }
  .playlist-backdrop {
    position: fixed;
    inset: 0;
    z-index: 15;
    display: block;
    background: rgba(77, 47, 40, 0.24);
  }
  .track-heading {
    position: relative;
    min-height: 65px;
    padding: 0 18px;
    border-bottom: 1px solid rgba(187, 132, 111, 0.14);
  }
  .track-heading::before {
    position: absolute;
    top: 8px;
    left: 50%;
    width: 36px;
    height: 4px;
    border-radius: 2px;
    background: #dfb7a6;
    content: "";
    transform: translateX(-50%);
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
    min-height: 64px;
    margin-top: 14px;
  }
  .player-options,
  .rate-options {
    display: none;
  }
  .playback-controls {
    grid-column: 2;
    grid-row: 1;
    gap: 20px;
  }
  .playback-controls button {
    width: 45px;
    height: 45px;
  }
  .playback-controls .play-button {
    width: 64px;
    height: 64px;
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
  .mobile-settings-action button,
  .mobile-playlist-action button {
    display: grid;
    width: 38px;
    height: 38px;
    border: 0;
    border-radius: 50%;
    color: #895c51;
    background: rgba(255, 242, 234, 0.9);
    place-items: center;
  }
  .mobile-settings-action button.active {
    color: #fff;
    background: var(--theme-color);
  }
  .mobile-player-settings {
    position: fixed;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 16;
    display: flex;
    align-items: stretch;
    flex-direction: column;
    gap: 14px;
    width: min(100%, 520px);
    margin: 0 auto;
    padding: 20px 18px calc(18px + env(safe-area-inset-bottom));
    border: 1px solid rgba(187, 132, 111, 0.16);
    border-bottom: 0;
    border-radius: 22px 22px 0 0;
    background: rgba(255, 250, 247, 0.88);
    box-shadow: 0 -12px 32px rgba(91, 54, 44, 0.16);
    backdrop-filter: blur(18px);
    -webkit-backdrop-filter: blur(18px);
  }
  .mobile-player-settings label {
    display: flex;
    min-width: 0;
    flex: 1;
    align-items: center;
    gap: 8px;
    color: #a17367;
  }
  .mobile-player-settings input {
    width: 100%;
    min-width: 48px;
    accent-color: var(--theme-color);
  }
  .mobile-player-settings label span {
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
}
</style>
