<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    open: boolean;
    backdropClass?: "playlist-backdrop" | "desktop-playlist-backdrop";
  }>(),
  {
    backdropClass: "playlist-backdrop",
  },
);

const emit = defineEmits<{
  close: [];
}>();
</script>

<template>
  <Transition name="drawer-backdrop">
    <div
      v-if="props.open"
      class="audio-drawer-backdrop"
      :class="props.backdropClass"
      aria-hidden="true"
      @click="emit('close')"
    />
  </Transition>
  <Transition name="audio-drawer">
    <slot v-if="props.open" />
  </Transition>
</template>

<style>
.audio-drawer-backdrop {
  position: fixed;
  z-index: 15;
  inset: 0;
  background: rgba(77, 47, 40, 0.24);
}
.audio-drawer-backdrop.desktop-playlist-backdrop {
  position: absolute;
  z-index: 2;
  background: rgba(48, 29, 36, 0.28);
  backdrop-filter: blur(2px);
  -webkit-backdrop-filter: blur(2px);
}
.drawer-backdrop-enter-active,
.drawer-backdrop-leave-active {
  transition: opacity 0.24s ease;
}
.drawer-backdrop-enter-from,
.drawer-backdrop-leave-to {
  opacity: 0;
}
.drawer-backdrop-enter-to,
.drawer-backdrop-leave-from {
  opacity: 1;
}
.audio-drawer-enter-active,
.audio-drawer-leave-active {
  transition:
    transform 0.3s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.22s ease;
  will-change: transform, opacity;
}
.audio-drawer-enter-from,
.audio-drawer-leave-to {
  opacity: 0;
  transform: translateX(100%);
}
.audio-drawer-enter-to,
.audio-drawer-leave-from {
  opacity: 1;
  transform: translateX(0);
}

@media (max-width: 800px) {
  .audio-drawer-backdrop.desktop-playlist-backdrop {
    display: none;
  }
  .audio-drawer-backdrop.playlist-backdrop {
    position: fixed;
    z-index: 15;
    background: rgba(77, 47, 40, 0.24);
  }
  .player-drawer {
    box-sizing: border-box;
    position: fixed;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 16;
    width: min(100%, 520px);
    max-height: min(78dvh, 620px);
    min-height: 0;
    overflow-y: auto;
    margin: 0 auto;
    border: 1px solid rgba(187, 132, 111, 0.16);
    border-bottom: 0;
    border-radius: 22px 22px 0 0;
    background: rgba(255, 250, 247, 0.88);
    box-shadow: 0 -12px 32px rgba(91, 54, 44, 0.16);
    backdrop-filter: blur(18px);
    -webkit-backdrop-filter: blur(18px);
    transform: translateY(0);
    will-change: transform, opacity;
  }
  .player-drawer::before {
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
  .player-drawer.audio-drawer-enter-from,
  .player-drawer.audio-drawer-leave-to {
    opacity: 0;
    transform: translateY(100%);
  }
  .player-drawer.audio-drawer-enter-to,
  .player-drawer.audio-drawer-leave-from {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
