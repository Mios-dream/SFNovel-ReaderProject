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
  Volume2,
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
const currentTrack = computed(() => props.tracks[trackIndex.value]);
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
        </div>
      </div>
      <aside class="track-section">
        <div class="track-heading">
          <span><ListMusic :size="19" />播放队列</span
          ><small>{{ tracks.length }} 章</small>
        </div>
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
  height: calc(100dvh - 130px);
  min-height: 0;
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
  height: calc(100% - 30px);
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
  padding: 0 20px 14px;
  color: #69453d;
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
    height: auto;
    min-height: 0;
    overflow: visible;
  }
  .music-player {
    grid-template-columns: 1fr;
    height: auto;
    min-height: 0;
  }
  .player-main {
    border-right: 0;
    border-bottom: 1px solid rgba(187, 132, 111, 0.2);
  }
  .track-section {
    max-height: 330px;
  }
  .cover-frame {
    width: min(220px, 52vw);
  }
  .track-list {
    max-height: 260px;
  }
}
@media (max-width: 500px) {
  .player-main {
    padding: 28px 18px 22px;
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
  .control-row {
    grid-template-columns: 1fr auto;
    gap: 12px;
  }
  .rate-options {
    grid-column: 1 / -1;
    grid-row: 2;
    justify-self: center;
  }
  .player-options input {
    width: 95px;
  }
  .track-section {
    padding-top: 19px;
  }
  .track-heading {
    padding: 0 15px 11px;
  }
}
</style>
