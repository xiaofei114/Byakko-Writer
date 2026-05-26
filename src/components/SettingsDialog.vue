<script setup lang="ts">
import { ref } from 'vue';
import { useConfigStore } from '../stores/config';
import {
  Setting,
  Grid,
  Cpu,
  EditPen,
  InfoFilled,
  Link,
  Sunny as SunnyIcon
} from '@element-plus/icons-vue';

const configStore = useConfigStore();
const activeTab = ref<'general' | 'ai'>('general');

const themes = [
  { value: 'light', label: '浅色' },
  { value: 'dark', label: '深色' },
  { value: 'auto', label: '跟随系统' }
];

const fonts = [
  { value: 'system', label: '系统默认' },
  { value: 'LXGWWenKai', label: '霞鹜文楷' },
  { value: 'NotoSerifCJK', label: '思源宋体' },
  { value: 'PingFang', label: '苹方' },
  { value: 'MicrosoftYaHei', label: '微软雅黑' },
  { value: 'SourceHanSans', label: '思源黑体' },
];

// AI 提供商配置预设
const aiProviderPresets: Record<string, {
  name: string;
  apiUrl: string;
  model: string;
  help?: string;
  keyUrl: string;
  showApiUrl?: boolean;
}> = {
  deepseek: {
    name: 'DeepSeek',
    apiUrl: 'https://api.deepseek.com/v1',
    model: 'deepseek-chat',
    help: 'DeepSeek 官方 API，价格便宜效果好',
    keyUrl: 'https://platform.deepseek.com/'
  },
  openai: {
    name: 'OpenAI',
    apiUrl: 'https://api.openai.com/v1',
    model: 'gpt-3.5-turbo',
    help: 'OpenAI 官方 API，需要海外支付方式',
    keyUrl: 'https://platform.openai.com/api-keys'
  },
  aliyun: {
    name: '阿里云 (通义千问)',
    apiUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    model: 'qwen-turbo',
    help: '阿里云灵积平台，国内访问稳定',
    keyUrl: 'https://dashscope.console.aliyun.com/apiKey'
  },
  moonshot: {
    name: '月之暗面 (Kimi)',
    apiUrl: 'https://api.moonshot.cn/v1',
    model: 'moonshot-v1-8k',
    help: 'Kimi 长文本模型，支持超长上下文',
    keyUrl: 'https://platform.moonshot.cn/'
  },
  zhipu: {
    name: '智谱AI (GLM)',
    apiUrl: 'https://open.bigmodel.cn/api/paas/v4',
    model: 'glm-4-flash',
    help: '智谱清言，国内大模型，有免费额度',
    keyUrl: 'https://open.bigmodel.cn/usercenter/apikeys'
  },
  ollama: {
    name: 'Ollama (本地)',
    apiUrl: 'http://localhost:11434/v1',
    model: 'llama3.2',
    help: '本地运行开源模型，无需联网和 API 密钥',
    keyUrl: 'https://ollama.com/',
    showApiUrl: true
  },
  custom: {
    name: '自定义',
    apiUrl: '',
    model: '',
    help: '其他兼容 OpenAI 格式的 API',
    keyUrl: '',
    showApiUrl: true
  }
};

const aiProviders = Object.entries(aiProviderPresets).map(([value, config]) => ({
  value,
  label: config.name,
  ...config
}));

// 切换提供商时自动填充预设值
const onProviderChange = (provider: string) => {
  const preset = aiProviderPresets[provider];
  if (preset && provider !== 'custom') {
    configStore.config.ai.apiUrl = preset.apiUrl;
    configStore.config.ai.model = preset.model;
  }
  configStore.saveConfig();
};

// 字体设置改变时应用并保存
const onFontChange = () => {
  configStore.applyTheme();
  configStore.saveConfig();
};

</script>

<template>
  <el-dialog
    v-model="configStore.isSettingsOpen"
    title="设置"
    width="520px"
    :close-on-click-modal="false"
    destroy-on-close
    class="settings-dialog"
  >
    <template #header>
      <div class="dialog-header">
        <el-icon :size="20"><Setting /></el-icon>
        <span>设置</span>
      </div>
    </template>

    <el-tabs v-model="activeTab" class="settings-tabs">
      <!-- 通用设置 -->
      <el-tab-pane name="general">
        <template #label>
          <span class="tab-label">
            <el-icon><Grid /></el-icon>
            <span>通用</span>
          </span>
        </template>

        <div class="settings-form">
          <!-- 外观 -->
          <el-divider content-position="left">
            <span class="divider-title">
              <el-icon><SunnyIcon /></el-icon>
              外观
            </span>
          </el-divider>

          <div class="form-row">
            <label class="form-label">主题</label>
            <el-select
              v-model="configStore.config.theme"
              @change="configStore.setTheme(configStore.config.theme)"
              style="width: 160px"
            >
              <el-option
                v-for="theme in themes"
                :key="theme.value"
                :label="theme.label"
                :value="theme.value"
              />
            </el-select>
          </div>

          <!-- 编辑器 -->
          <el-divider content-position="left">
            <span class="divider-title">
              <el-icon><EditPen /></el-icon>
              编辑器
            </span>
          </el-divider>

          <div class="form-row">
            <label class="form-label">字体</label>
            <el-select
              v-model="configStore.config.fontFamily"
              @change="onFontChange"
              style="width: 180px"
            >
              <el-option
                v-for="font in fonts"
                :key="font.value"
                :label="font.label"
                :value="font.value"
              />
            </el-select>
          </div>

          <div class="form-row">
            <label class="form-label">字体大小</label>
            <el-input-number
              v-model="configStore.config.fontSize"
              :min="12"
              :max="32"
              :step="1"
              @change="onFontChange"
            />
            <span class="unit">px</span>
          </div>

          <div class="form-row">
            <label class="form-label">行高</label>
            <el-input-number
              v-model="configStore.config.lineHeight"
              :min="1"
              :max="3"
              :step="0.1"
              @change="onFontChange"
            />
          </div>

          <!-- 自动保存 -->
          <el-divider content-position="left">
            <span class="divider-title">自动保存</span>
          </el-divider>

          <div class="form-row">
            <label class="form-label">启用</label>
            <el-switch
              v-model="configStore.config.autoSave"
              @change="configStore.saveConfig"
            />
          </div>

          <div class="form-row" v-if="configStore.config.autoSave">
            <label class="form-label">间隔</label>
            <el-slider
              v-model="configStore.config.autoSaveInterval"
              :min="10"
              :max="300"
              :step="10"
              @change="configStore.saveConfig"
              style="width: 200px"
            />
            <span class="unit">{{ configStore.config.autoSaveInterval }}秒</span>
          </div>
        </div>
      </el-tab-pane>

      <!-- AI 设置 -->
      <el-tab-pane name="ai">
        <template #label>
          <span class="tab-label">
            <el-icon><Cpu /></el-icon>
            <span>AI</span>
          </span>
        </template>

        <div class="settings-form">
          <!-- AI 服务 -->
          <el-divider content-position="left">
            <span class="divider-title">
              <el-icon><Cpu /></el-icon>
              AI 服务
            </span>
          </el-divider>

          <div class="form-row">
            <label class="form-label">服务商</label>
            <el-select
              v-model="configStore.config.ai.provider"
              @change="onProviderChange"
              style="width: 280px"
            >
              <el-option
                v-for="provider in aiProviders"
                :key="provider.value"
                :label="provider.label"
                :value="provider.value"
              />
            </el-select>
          </div>

          <div class="form-help" v-if="aiProviderPresets[configStore.config.ai.provider]?.help">
            <el-icon><InfoFilled /></el-icon>
            <span>{{ aiProviderPresets[configStore.config.ai.provider].help }}</span>
          </div>

          <div class="form-row" v-if="configStore.config.ai.provider !== 'ollama'">
            <label class="form-label">API Key</label>
            <el-input
              v-model="configStore.config.ai.apiKey"
              type="password"
              placeholder="输入 API 密钥"
              @change="configStore.saveConfig"
              style="width: 280px"
              show-password
            />
          </div>

          <div class="form-link" v-if="aiProviderPresets[configStore.config.ai.provider]?.keyUrl">
            <a
              :href="aiProviderPresets[configStore.config.ai.provider].keyUrl"
              target="_blank"
              rel="noopener noreferrer"
            >
              <el-icon><Link /></el-icon>
              获取 API 密钥
            </a>
          </div>

          <div class="form-row" v-if="aiProviderPresets[configStore.config.ai.provider]?.showApiUrl">
            <label class="form-label">API 地址</label>
            <el-input
              v-model="configStore.config.ai.apiUrl"
              placeholder="https://api.example.com/v1"
              @change="configStore.saveConfig"
              style="width: 280px"
            />
          </div>

          <div class="form-row">
            <label class="form-label">模型</label>
            <el-input
              v-model="configStore.config.ai.model"
              placeholder="模型名称"
              @change="configStore.saveConfig"
              style="width: 280px"
            />
          </div>

          <!-- 生成参数 -->
          <el-divider content-position="left">
            <span class="divider-title">生成参数</span>
          </el-divider>

          <div class="form-row">
            <label class="form-label">温度</label>
            <el-slider
              v-model="configStore.config.ai.temperature"
              :min="0"
              :max="2"
              :step="0.1"
              @change="configStore.saveConfig"
              style="width: 200px"
            />
            <span class="unit">{{ configStore.config.ai.temperature }}</span>
          </div>

          <div class="form-row">
            <label class="form-label">最大 Token</label>
            <el-input-number
              v-model="configStore.config.ai.maxTokens"
              :min="100"
              :max="4000"
              :step="100"
              @change="configStore.saveConfig"
            />
          </div>

          <div class="form-row">
            <label class="form-label">最大决策轮次</label>
            <el-input-number
              v-model="configStore.config.ai.maxRounds"
              :min="3"
              :max="20"
              :step="1"
              @change="configStore.saveConfig"
            />
            <span class="form-hint">AI 调用工具的最大轮次数，越大上下文越完整但耗时越长</span>
          </div>
        </div>
      </el-tab-pane>
    </el-tabs>
  </el-dialog>
</template>

<style scoped>
.dialog-header {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 16px;
  font-weight: 600;
  color: var(--nw-text-primary);
}

.settings-tabs :deep(.el-tabs__header) {
  margin-bottom: 20px;
}

.tab-label {
  display: flex;
  align-items: center;
  gap: 4px;
}

.settings-form {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

/* 分割线标题 */
.divider-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 600;
  color: var(--nw-text-secondary);
}

.divider-title .el-icon {
  font-size: 14px;
  color: var(--nw-primary);
}

/* 表单行 */
.form-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 4px 0;
}

.form-label {
  width: 70px;
  font-size: 13px;
  color: var(--nw-text-secondary);
  flex-shrink: 0;
}

.unit {
  font-size: 13px;
  color: var(--nw-text-tertiary);
  min-width: 40px;
}

.form-help {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: -4px 0 4px 82px;
  font-size: 12px;
  color: var(--nw-text-tertiary);
}

.form-help .el-icon {
  font-size: 14px;
  color: var(--nw-accent);
}

.form-link {
  margin: -4px 0 4px 82px;
}

.form-link a {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--nw-primary);
  text-decoration: none;
}

.form-link a:hover {
  text-decoration: underline;
}
</style>
