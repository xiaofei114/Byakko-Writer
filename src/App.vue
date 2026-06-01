<script setup lang="ts">
import { computed, watch } from 'vue';
import { useBookStore } from './stores/book';
import { useConfigStore } from './stores/config';
import Home from './views/Home.vue';
import Editor from './views/Editor.vue';
import SettingsDialog from './components/SettingsDialog.vue';
import { FONT_FAMILY_MAP } from './config/appConfig';

const bookStore = useBookStore();
const configStore = useConfigStore();

const showEditor = computed(() => bookStore.currentBook !== null);

// 应用全局字体
const applyGlobalFont = (fontKey: string) => {
  const fontFamily = FONT_FAMILY_MAP[fontKey] || FONT_FAMILY_MAP['system'];
  document.documentElement.style.setProperty('--app-font-family', fontFamily);
};

// 监听字体变化
watch(() => configStore.config.fontFamily, (newFont) => {
  applyGlobalFont(newFont);
}, { immediate: true });
</script>

<template>
  <div class="app">
    <Home v-if="!showEditor" />
    <Editor v-else />
    <SettingsDialog />
  </div>
</template>

<style>
.app {
  height: 100vh;
  width: 100vw;
  overflow: hidden;
}
</style>
