<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
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
import type { LocalAudioTrack } from "../types";

const props = defineProps<{
  bookName: string;
  author: string;
  cover?: string;
  tracks: LocalAudioTrack[];
  initialTrackIndex?: number;
}>();
const emit = defineEmits<{ back: [] }>();

const audio = ref<HTMLAudioElement>();
const trackIndex = ref(0);
const playing = ref(false);
const currentTime = ref(0);
const timelinePosition = ref(0);
const seeking = ref(false);
const duration = ref(0);
const volume = ref(0.8);
const playbackRate = ref(1);
const mobileSettingsOpen = ref(false);
const playlistOpen = ref(false);
const currentTrack = computed(() => props.tracks[trackIndex.value]);

function closeMobileDrawers() {
  mobileSettingsOpen.value = false;
  playlistOpen.value = false;
}

function toggleMobileSettings() {
  if (mobileSettingsOpen.value) closeMobileDrawers();
  else {
    playlistOpen.value = false;
    mobileSettingsOpen.value = true;
  }
}

function openMobilePlaylist() {
  mobileSettingsOpen.value = false;
  playlistOpen.value = true;
}
/**
 * Converts a playback duration into the compact minutes-and-seconds label.
 *
 * @param seconds Duration in seconds; invalid or negative values become zero.
 * @returns A user-facing `m:ss` duration string.
 */
const formatTime = (seconds: number) => {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${String(Math.floor(seconds % 60)).padStart(2, "0")}`;
};

/**
 * Starts playback of the currently selected native asset track.
 *
 * @returns A promise settled after the browser media element accepts or
 * rejects playback; rejection leaves the player paused.
 */
async function play() {
  if (!audio.value) return;
  try {
    await audio.value.play();
    playing.value = true;
  } catch {
    playing.value = false;
  }
}

/**
 * Pauses the current audio element and updates the reactive playback state.
 *
 * @returns No value.
 */
function pause() {
  audio.value?.pause();
  playing.value = false;
}

/**
 * Toggles between play and pause using the current media state.
 *
 * @returns No value; playback is started asynchronously when needed.
 */
function togglePlayback() {
  if (playing.value) pause();
  else void play();
}

/**
 * Selects a bounded track index and resets position metadata for the track.
 *
 * @param index Requested track index, clamped to the available track range.
 * @param shouldPlay Whether the selected track should start after the DOM
 * audio source has updated.
 * @returns No value.
 */
function selectTrack(index: number, shouldPlay = playing.value) {
  trackIndex.value = Math.min(Math.max(0, index), props.tracks.length - 1);
  currentTime.value = 0;
  timelinePosition.value = 0;
  seeking.value = false;
  duration.value = 0;
  void nextTick(() => {
    if (shouldPlay) void play();
  });
}

/**
 * Selects the previous track, wrapping from the first track to the last.
 *
 * @returns No value.
 */
function previous() {
  selectTrack(
    trackIndex.value > 0 ? trackIndex.value - 1 : props.tracks.length - 1,
  );
}

/**
 * Selects the next track, wrapping from the last track to the first.
 *
 * @returns No value.
 */
function next() {
  selectTrack(
    trackIndex.value < props.tracks.length - 1 ? trackIndex.value + 1 : 0,
  );
}

/**
 * Applies a range-input position immediately to the media element.
 *
 * @param event Input event emitted by the timeline range control.
 * @returns No value; invalid numeric input is ignored.
 */
function updateTimeline(event: Event) {
  const value = Number((event.target as HTMLInputElement).value);
  if (!Number.isFinite(value)) return;
  timelinePosition.value = value;
  currentTime.value = value;
  if (audio.value) audio.value.currentTime = value;
}

/**
 * Marks timeline dragging as active and captures the current position.
 *
 * @returns No value.
 */
function startSeeking() {
  seeking.value = true;
  timelinePosition.value = currentTime.value;
}

/**
 * Commits the dragged timeline position and ends seeking mode.
 *
 * @returns No value; does nothing when no seek operation is active.
 */
function finishSeeking() {
  if (!seeking.value) return;
  if (audio.value) audio.value.currentTime = timelinePosition.value;
  currentTime.value = timelinePosition.value;
  seeking.value = false;
}

/**
 * Synchronizes the reactive position with the media element clock.
 *
 * @returns No value; updates are skipped while the user is dragging.
 */
function syncPlaybackTime() {
  if (!audio.value || seeking.value) return;
  currentTime.value = audio.value.currentTime;
  timelinePosition.value = audio.value.currentTime;
}

/**
 * Reads the loaded media duration and refreshes the displayed timeline.
 *
 * @returns No value.
 */
function updateDuration() {
  duration.value = audio.value?.duration || 0;
  syncPlaybackTime();
}

/**
 * Applies the volume slider value to the media element.
 *
 * @param event Input event emitted by the volume range control.
 * @returns No value.
 */
function changeVolume(event: Event) {
  volume.value = Number((event.target as HTMLInputElement).value);
  if (audio.value) audio.value.volume = volume.value;
}

/**
 * Sets the media playback rate for the current and future tracks.
 *
 * @param rate Playback multiplier selected by the user.
 * @returns No value.
 */
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
  () => props.initialTrackIndex,
  (index) => {
    if (!props.tracks.length) return;
    selectTrack(index || 0, false);
  },
  { immediate: true },
);
</script>

<template>
  <section class="audio-player-page">
    <button class="back-button" @click="emit('back')">
      <ChevronLeft :size="17" />{{ bookName }}
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
          <p>本地有声书</p>
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
      </div>
      <aside class="track-section" :class="{ 'queue-open': playlistOpen }">
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
  </section>
</template>

<style scoped>
.audio-player-page {
  display: flex;
  height: 100%;
  min-height: 0;
  flex-direction: column;
  padding: 5px 8px 10px;
  overflow: hidden;
}
.back-button {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  margin-bottom: 16px;
  padding: 0;
  border: 0;
  color: #94675c;
  background: none;
  font-size: 12px;
  font-weight: 600;
}
.music-player {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(280px, 360px);
  height: auto;
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
  border: 1px solid rgba(224, 176, 154, 0.8);
  border-radius: 8px;
  background: rgba(255, 250, 247, 0.78);
  /* box-shadow: 0 18px 42px rgba(137, 76, 55, 0.13); */
}
.player-main {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  align-items: center;
  padding: clamp(18px, 3vh, 34px) clamp(24px, 6vw, 78px) 18px;
  border-right: 1px solid rgba(187, 132, 111, 0.2);
  background: linear-gradient(
    150deg,
    rgba(255, 255, 255, 0.68),
    rgba(255, 236, 226, 0.65)
  );
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
.mobile-player-settings,
.mobile-settings-action,
.mobile-playlist-action,
.playlist-backdrop {
  display: none;
}
.track-section {
  display: flex;
  min-height: 0;
  flex-direction: column;
  padding: 24px 0 0;
  background: rgba(255, 253, 251, 0.6);
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
.drawer-close {
  display: none;
}
.drawer-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
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
@media (max-width: 800px) {
  .audio-player-page {
    min-height: calc(100dvh - env(safe-area-inset-top));
    /* padding: 0 4px calc(92px + env(safe-area-inset-bottom)); */
    overflow: visible;
  }
  .back-button {
    min-height: 42px;
    margin-bottom: 8px;
  }
  .music-player {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    height: auto;
    min-height: 0;
    overflow: visible;
    border: 0;
    background: none;
  }
  .player-main {
    flex: 1 1 auto;
    border: 1px solid rgba(224, 176, 154, 0.8);
    border-radius: 8px;
    border-right: 0;
    background: linear-gradient(
      150deg,
      rgba(255, 255, 255, 0.76),
      rgba(255, 236, 226, 0.7)
    );
  }
  .track-section {
    position: fixed;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 16;
    height: min(70dvh, 510px);
    min-height: 0;
    padding: 0;
    border: 1px solid rgba(224, 176, 154, 0.9);
    border-bottom: 0;
    border-radius: 14px 14px 0 0;
    background: rgba(255, 250, 247, 0.98);
    box-shadow: 0 -12px 32px rgba(91, 54, 44, 0.16);
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
  .track-list {
    padding-bottom: env(safe-area-inset-bottom);
  }
  .cover-frame {
    width: min(220px, 52vw);
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
    border-radius: 14px 14px 0 0;
    background: rgba(255, 250, 247, 0.98);
    box-shadow: 0 -12px 32px rgba(91, 54, 44, 0.16);
  }
  .drawer-heading {
    margin-bottom: 2px;
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
@media (max-width: 500px) {
  .player-main {
    padding: 20px 18px 26px;
  }
  .cover-frame {
    width: min(190px, 56vw);
    border-width: 7px;
  }
  .book-meta h2 {
    font-size: 21px;
  }
  .now-playing {
    margin-top: 23px;
  }
  .playback-controls {
    gap: 14px;
  }
  .mobile-settings-action button,
  .mobile-playlist-action button {
    width: 34px;
    height: 34px;
  }
}
</style>
