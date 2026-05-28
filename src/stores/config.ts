import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, AIConfig } from '../types';

// 后端配置格式
interface BackendConfig {
  theme: string;
  primary_color: string;
  font_family: string;
  font_size: number;
  line_height: number;
  auto_save: boolean;
  auto_save_interval: number;
  auto_story_memory?: boolean;
  ai: {
    provider: string;
    api_key: string;
    api_url: string;
    model: string;
    temperature: number;
    max_tokens: number;
    max_rounds: number;
  };
}

const defaultConfig: AppConfig = {
  theme: 'light',
  primaryColor: '#3498db',
  fontFamily: 'system',
  fontSize: 16,
  lineHeight: 1.8,
  autoSave: true,
  autoSaveInterval: 30,
  autoStoryMemory: true,
  ai: {
    provider: 'deepseek',
    apiKey: '',
    apiUrl: 'https://api.deepseek.com/v1',
    model: 'deepseek-chat',
    temperature: 0.7,
    maxTokens: 2000,
    maxRounds: 30
  }
};

export const useConfigStore = defineStore('config', () => {
  // State
  const config = ref<AppConfig>({ ...defaultConfig });
  const isSettingsOpen = ref(false);
  const isLoading = ref(false);

  // Getters
  const theme = computed(() => config.value.theme);
  const primaryColor = computed(() => config.value.primaryColor);
  const aiConfig = computed(() => config.value.ai);
  const autoSaveEnabled = computed(() => config.value.autoSave);
  const autoSaveIntervalMs = computed(() => config.value.autoSaveInterval * 1000);

  // 转换后端配置到前端格式
  const convertFromBackend = (backend: BackendConfig): AppConfig => {
    return {
      theme: backend.theme as 'light' | 'dark' | 'auto',
      primaryColor: backend.primary_color,
      fontFamily: backend.font_family || 'LXGWWenKai',
      fontSize: backend.font_size,
      lineHeight: backend.line_height,
      autoSave: backend.auto_save,
      autoSaveInterval: backend.auto_save_interval,
      autoStoryMemory: backend.auto_story_memory ?? true,
      ai: {
        provider: backend.ai.provider,
        apiKey: backend.ai.api_key,
        apiUrl: backend.ai.api_url,
        model: backend.ai.model,
        temperature: backend.ai.temperature,
        maxTokens: backend.ai.max_tokens,
        maxRounds: backend.ai.max_rounds || 30
      }
    };
  };

  // 转换前端配置到后端格式
  const convertToBackend = (cfg: AppConfig): BackendConfig => {
    return {
      theme: cfg.theme,
      primary_color: cfg.primaryColor,
      font_family: cfg.fontFamily,
      font_size: cfg.fontSize,
      line_height: cfg.lineHeight,
      auto_save: cfg.autoSave,
      auto_save_interval: cfg.autoSaveInterval,
      auto_story_memory: cfg.autoStoryMemory,
      ai: {
        provider: cfg.ai.provider,
        api_key: cfg.ai.apiKey,
        api_url: cfg.ai.apiUrl,
        model: cfg.ai.model,
        temperature: cfg.ai.temperature,
        max_tokens: cfg.ai.maxTokens,
        max_rounds: cfg.ai.maxRounds
      }
    };
  };

  // Actions
  const loadConfig = async () => {
    isLoading.value = true;
    try {
      const backendConfig = await invoke<BackendConfig>('load_config');
      config.value = convertFromBackend(backendConfig);
      applyTheme();
    } catch (error) {
      console.error('加载配置失败:', error);
      // 使用默认配置
      config.value = { ...defaultConfig };
    } finally {
      isLoading.value = false;
    }
  };

  const saveConfig = async () => {
    try {
      const backendConfig = convertToBackend(config.value);
      await invoke('save_config', { config: backendConfig });
    } catch (error) {
      console.error('保存配置失败:', error);
    }
  };

  const updateConfig = async (partial: Partial<AppConfig>) => {
    config.value = { ...config.value, ...partial };
    await saveConfig();
  };

  const updateAIConfig = async (partial: Partial<AIConfig>) => {
    config.value.ai = { ...config.value.ai, ...partial };
    await saveConfig();
  };

  const setTheme = async (themeValue: AppConfig['theme']) => {
    config.value.theme = themeValue;
    await saveConfig();
    applyTheme();
  };

  const setPrimaryColor = async (color: string) => {
    config.value.primaryColor = color;
    await saveConfig();
    applyTheme();
  };

  const applyTheme = () => {
    const root = document.documentElement;
    const html = document.documentElement;
    const primaryColor = config.value.primaryColor;

    // 应用主色调 - 更新 Element Plus 的主色变量
    root.style.setProperty('--el-color-primary', primaryColor);

    // 计算主色的变体
    root.style.setProperty('--el-color-primary-dark-2', adjustColor(primaryColor, -20));
    root.style.setProperty('--el-color-primary-light-3', adjustColor(primaryColor, 30));
    root.style.setProperty('--el-color-primary-light-5', adjustColor(primaryColor, 50));
    root.style.setProperty('--el-color-primary-light-7', adjustColor(primaryColor, 70));
    root.style.setProperty('--el-color-primary-light-8', adjustColor(primaryColor, 80));
    root.style.setProperty('--el-color-primary-light-9', adjustColor(primaryColor, 95));

    // 同时更新自定义 CSS 变量
    root.style.setProperty('--nw-primary', primaryColor);
    root.style.setProperty('--nw-primary-light', adjustColor(primaryColor, 30));
    root.style.setProperty('--nw-primary-lighter', adjustColor(primaryColor, 60));
    root.style.setProperty('--nw-primary-dark', adjustColor(primaryColor, -20));

    // 应用字体设置
    const fontFamily = config.value.fontFamily;
    if (fontFamily && fontFamily !== 'system') {
      root.style.setProperty('--nw-font-body', `"${fontFamily}", "Noto Sans SC", "PingFang SC", "Microsoft YaHei", sans-serif`);
      root.style.setProperty('--el-font-family', `"${fontFamily}", "Noto Sans SC", "PingFang SC", "Microsoft YaHei", sans-serif`);
    } else {
      // 系统默认字体
      root.style.setProperty('--nw-font-body', '"Noto Sans SC", "PingFang SC", "Microsoft YaHei", sans-serif');
      root.style.setProperty('--el-font-family', '"Noto Sans SC", "PingFang SC", "Microsoft YaHei", sans-serif');
    }

    // 应用字体大小
    root.style.setProperty('--nw-font-size', `${config.value.fontSize}px`);
    root.style.setProperty('--nw-line-height', config.value.lineHeight.toString());

    // 应用深色/浅色主题
    html.classList.remove('dark');

    let effectiveTheme = config.value.theme;
    if (effectiveTheme === 'auto') {
      effectiveTheme = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }

    if (effectiveTheme === 'dark') {
      html.classList.add('dark');
    }
  };
  
  // 辅助函数：调整颜色亮度
  const adjustColor = (color: string, amount: number): string => {
    const hex = color.replace('#', '');
    const r = Math.max(0, Math.min(255, parseInt(hex.substring(0, 2), 16) + amount));
    const g = Math.max(0, Math.min(255, parseInt(hex.substring(2, 4), 16) + amount));
    const b = Math.max(0, Math.min(255, parseInt(hex.substring(4, 6), 16) + amount));
    return `#${r.toString(16).padStart(2, '0')}${g.toString(16).padStart(2, '0')}${b.toString(16).padStart(2, '0')}`;
  };

  const openSettings = () => {
    isSettingsOpen.value = true;
  };

  const closeSettings = () => {
    isSettingsOpen.value = false;
  };

  // 初始化
  const init = async () => {
    await loadConfig();
  };

  return {
    config,
    isSettingsOpen,
    isLoading,
    theme,
    primaryColor,
    aiConfig,
    autoSaveEnabled,
    autoSaveIntervalMs,
    loadConfig,
    saveConfig,
    updateConfig,
    updateAIConfig,
    setTheme,
    setPrimaryColor,
    applyTheme,
    openSettings,
    closeSettings,
    init
  };
});
