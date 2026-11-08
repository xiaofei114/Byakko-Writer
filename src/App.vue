<script setup lang="ts">
import { computed } from 'vue';
import { useBookStore } from './stores/book';
import Home from './views/Home.vue';
import Editor from './views/Editor.vue';
import SettingsDialog from './components/SettingsDialog.vue';

const bookStore = useBookStore();

const showEditor = computed(() => bookStore.currentBook !== null);
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
