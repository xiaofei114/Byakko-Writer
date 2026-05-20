<script setup lang="ts">
import { computed, watch } from 'vue';
import { useBookStore } from './stores/book';
import { useConfigStore } from './stores/config';
import Home from './views/Home.vue';
import Editor from './views/Editor.vue';
import SettingsDialog from './components/SettingsDialog.vue';

const bookStore = useBookStore();
const configStore = useConfigStore();

const showEditor = computed(() => bookStore.currentBook !== null);

// 字体映射
const fontFamilyMap: Record<string, string> = {
  'system': 'system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif',
  'LXGWWenKai': '"LXGWWenKai", "Noto Serif SC", "Source Han Serif SC", serif',
  'NotoSerifCJK': '"NotoSerifCJK", "Noto Serif SC", "Source Han Serif SC", serif',
  'EBGaramond': '"EBGaramond", "Times New Roman", Georgia, serif',
  'PingFang': '"PingFang SC", "Hiragino Sans GB", sans-serif',
  'MicrosoftYaHei': '"Microsoft YaHei", "PingFang SC", sans-serif',
  'SourceHanSans': '"Source Han Sans SC", "Noto Sans SC", sans-serif',
  'Lora': '"Lora", Georgia, serif',
  'JetBrainsMono': '"JetBrains Mono", monospace'
};

// 应用全局字体
const applyGlobalFont = (fontKey: string) => {
  const fontFamily = fontFamilyMap[fontKey] || fontFamilyMap['system'];
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
