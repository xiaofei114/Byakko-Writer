<script setup lang="ts">
import { ref, watch } from 'vue';
import { useConfigStore } from '../stores/config';
import { useBookStore } from '../stores/book';
import {
  Setting,
  Grid,
  Cpu,
  EditPen,
  InfoFilled,
  Link,
  Sunny as SunnyIcon,
  Collection,
  Refresh,
  Delete,
  Warning,
} from '@element-plus/icons-vue';
import { ElMessage } from 'element-plus';
import { getStoryMemoryText, updateStoryMemory, forceRegenerateStoryMemory } from '../api/storyMemory';
import type { StoryMemoryUpdateResult } from '../types';

const configStore = useConfigStore();
const bookStore = useBookStore();
const activeTab = ref<'general' | 'ai' | 'storyMemory'>('general');

// 故事记忆状态
const storyMemoryText = ref('');
const storyMemoryLoading = ref(false);
const storyMemoryResult = ref<StoryMemoryUpdateResult | null>(null);
const showForceRegenerateConfirm = ref(false);

// 处理强制重新生成
const handleForceRegenerate = async () => {
  if (!bookStore.currentBook) return;
  showForceRegenerateConfirm.value = false;
  storyMemoryLoading.value = true;
  storyMemoryResult.value = null;
  try {
    storyMemoryResult.value = await forceRegenerateStoryMemory(bookStore.currentBook.id);
    if (storyMemoryResult.value.success) {
      ElMessage.success('故事记忆已强制重新生成');
      await loadStoryMemory();
    } else {
      ElMessage.error(storyMemoryResult.value.message);
    }
  } catch (e) {
    ElMessage.error(`强制重新生成失败: ${e}`);
  } finally {
    storyMemoryLoading.value = false;
  }
};

const loadStoryMemory = async () => {
  if (!bookStore.currentBook) return;
  storyMemoryLoading.value = true;
  try {
    storyMemoryText.value = await getStoryMemoryText(bookStore.currentBook.id);
  } catch (e) {
    storyMemoryText.value = '';
  } finally {
    storyMemoryLoading.value = false;
  }
};

const handleRefreshStoryMemory = async () => {
  if (!bookStore.currentBook) return;
  storyMemoryLoading.value = true;
  storyMemoryResult.value = null;
  try {
    storyMemoryResult.value = await updateStoryMemory(bookStore.currentBook.id);
    if (storyMemoryResult.value.success) {
      ElMessage.success(storyMemoryResult.value.message);
      await loadStoryMemory();
    } else {
      ElMessage.error(storyMemoryResult.value.message);
    }
  } catch (e) {
    ElMessage.error(`更新失败: ${e}`);
  } finally {
    storyMemoryLoading.value = false;
  }
};

// 打开设置时自动加载
watch(() => configStore.isSettingsOpen, (open) => {
  if (open && bookStore.currentBook) {
    loadStoryMemory();
  }
});

// 切换到故事记忆 tab 时加载
watch(activeTab, (tab) => {
  if (tab === 'storyMemory' && bookStore.currentBook) {
    loadStoryMemory();
  }
});

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
    top="5vh"
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
              :min="20"
              :max="50"
              :step="1"
              @change="configStore.saveConfig"
            />
          </div>
          <div class="form-hint-row">
            <span class="form-hint">AI 调用工具的最大轮次数，可设置为 20-50；越大上下文越完整但耗时越长</span>
          </div>
        </div>
      </el-tab-pane>

      <!-- 故事记忆 -->
      <el-tab-pane name="storyMemory">
        <template #label>
          <span class="tab-label">
            <el-icon><Collection /></el-icon>
            <span>故事记忆</span>
          </span>
        </template>

        <div class="settings-form">
          <el-divider content-position="left">
            <span class="divider-title">AI 故事记忆（Story Bible）</span>
          </el-divider>

          <div class="story-memory-hint" v-if="!bookStore.currentBook">
            <el-icon><InfoFilled /></el-icon>
            <span>请先打开一本书，再查看故事记忆</span>
          </div>

          <template v-else>
            <!-- 自动故事记忆开关 -->
            <div class="auto-story-memory-toggle">
              <el-switch
                v-model="configStore.config.autoStoryMemory"
                @change="configStore.saveConfig"
                active-text="自动更新故事记忆"
              />
              <span class="toggle-hint">每30分钟自动检查并更新故事记忆</span>
            </div>

            <!-- 操作按钮区域 -->
            <div class="story-memory-actions">
              <div class="action-row">
                <el-button
                  type="primary"
                  :icon="Refresh"
                  :loading="storyMemoryLoading"
                  @click="handleRefreshStoryMemory"
                >
                  刷新故事记忆
                </el-button>
                <el-button
                  type="danger"
                  plain
                  :icon="Delete"
                  :loading="storyMemoryLoading"
                  @click="showForceRegenerateConfirm = true"
                >
                  强制重新生成
                </el-button>
              </div>
              <div class="action-hint">
                <p><strong>刷新</strong>：使用已有缓存，仅更新缺失部分，速度快</p>
                <p><strong>强制重新生成</strong>：清除所有缓存，重新分析所有章节，速度慢但质量更高</p>
              </div>
            </div>

            <!-- 确认对话框 -->
            <el-dialog
              v-model="showForceRegenerateConfirm"
              title="确认强制重新生成"
              width="400px"
              :close-on-click-modal="false"
            >
              <div class="confirm-content">
                <el-icon :size="40" color="#f56c6c"><Warning /></el-icon>
                <p>此操作将：</p>
                <ul>
                  <li>清除所有章节摘要缓存</li>
                  <li>清除所有分组总结缓存</li>
                  <li>重新生成所有内容（可能需要几分钟）</li>
                </ul>
                <p class="warning-text">确定要继续吗？</p>
              </div>
              <template #footer>
                <el-button @click="showForceRegenerateConfirm = false">取消</el-button>
                <el-button type="danger" @click="handleForceRegenerate">确定重新生成</el-button>
              </template>
            </el-dialog>

            <!-- 生成结果 -->
            <div class="story-memory-result" v-if="storyMemoryResult">
              <el-alert
                :type="storyMemoryResult.success ? 'success' : 'error'"
                :title="storyMemoryResult.message"
                :closable="true"
                @close="storyMemoryResult = null"
              />
              <!-- 统计信息 -->
              <div class="stats-row" v-if="storyMemoryResult.success">
                <el-statistic title="章节数" :value="storyMemoryResult.chapter_count" />
                <el-statistic title="总字数" :value="storyMemoryResult.total_word_count" />
                <el-statistic title="缓存分组" :value="storyMemoryResult.groups_cached" />
                <el-statistic title="生成分组" :value="storyMemoryResult.groups_generated" />
              </div>
              <!-- 分组进度 -->
              <div class="group-progress" v-if="storyMemoryResult.groups && storyMemoryResult.groups.length > 0">
                <div class="group-progress-title">分组处理详情：</div>
                <div
                  v-for="g in storyMemoryResult.groups"
                  :key="g.group_index"
                  class="group-progress-item"
                  :class="'status-' + g.status"
                >
                  <span class="group-tag">
                    <el-tag
                      :type="g.status === 'cached' ? 'info' : g.status === 'generated' ? 'success' : 'danger'"
                      size="small"
                    >
                      {{ g.status === 'cached' ? '缓存' : g.status === 'generated' ? '生成' : '失败' }}
                    </el-tag>
                  </span>
                  <span class="group-msg">{{ g.message }}</span>
                </div>
              </div>
            </div>

            <el-divider content-position="left">
              <span class="divider-title">当前故事记忆内容</span>
            </el-divider>

            <div class="story-memory-text" v-loading="storyMemoryLoading">
              <pre v-if="storyMemoryText">{{ storyMemoryText }}</pre>
              <div class="story-memory-empty" v-else-if="!storyMemoryLoading">
                <el-icon><InfoFilled /></el-icon>
                <span>尚未生成故事记忆。请先为章节生成摘要，然后点击「刷新故事记忆」按钮。</span>
              </div>
            </div>
          </template>
        </div>
      </el-tab-pane>
    </el-tabs>
  </el-dialog>
</template>

<style scoped>
/* 对话框整体 - 约束高度，body可滚动，header固定 */
.settings-dialog :deep(.el-dialog) {
  max-height: 92vh;
  display: flex;
  flex-direction: column;
}
.settings-dialog :deep(.el-dialog__header) {
  flex-shrink: 0;
}
.settings-dialog :deep(.el-dialog__body) {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
  padding-top: 0;
}

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

.form-hint-row {
  margin: -2px 0 4px 82px;
}

/* ===== 故事记忆 ===== */
.story-memory-hint {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 0;
  font-size: 13px;
  color: var(--nw-text-tertiary);
}

.story-memory-hint .el-icon {
  font-size: 16px;
  color: var(--nw-accent);
}

.form-hint {
  font-size: 12px;
  color: var(--nw-text-tertiary);
  max-width: 280px;
}

.story-memory-result {
  margin: 8px 0;
}

.story-memory-text {
  max-height: 280px;
  overflow: hidden;
  background: var(--nw-bg-secondary);
  border-radius: var(--nw-radius-sm);
}

.story-memory-text pre {
  margin: 0;
  padding: var(--nw-space-md);
  font-size: 13px;
  line-height: 1.7;
  color: var(--nw-text-primary);
  white-space: pre-wrap;
  word-break: break-word;
  font-family: var(--nw-font-body);
  max-height: 280px;
  overflow-y: auto;
}

.story-memory-empty {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--nw-text-tertiary);
  padding: var(--nw-space-md);
}

.group-progress {
  margin-top: 12px;
  max-height: 200px;
  overflow-y: auto;
}

.group-progress-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
  font-size: 12px;
}

.group-tag {
  flex-shrink: 0;
  width: 48px;
}

.group-msg {
  color: var(--nw-text-secondary);
}

/* ===== 自动故事记忆开关 ===== */
.auto-story-memory-toggle {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
  padding: 12px;
  background: var(--nw-bg-tertiary);
  border-radius: var(--nw-radius-sm);
}

.toggle-hint {
  font-size: 12px;
  color: var(--nw-text-tertiary);
}

/* ===== 故事记忆操作区域 ===== */
.story-memory-actions {
  background: var(--nw-bg-secondary);
  border-radius: var(--nw-radius-md);
  padding: var(--nw-space-md);
  margin-bottom: var(--nw-space-md);
}

.action-row {
  display: flex;
  gap: 12px;
  margin-bottom: 12px;
}

.action-hint {
  font-size: 12px;
  color: var(--nw-text-tertiary);
  line-height: 1.6;
}

.action-hint p {
  margin: 4px 0;
}

/* 确认对话框 */
.confirm-content {
  text-align: center;
  padding: 16px 0;
}

.confirm-content .el-icon {
  margin-bottom: 16px;
}

.confirm-content ul {
  text-align: left;
  display: inline-block;
  margin: 12px 0;
  padding-left: 20px;
  color: var(--nw-text-secondary);
}

.confirm-content li {
  margin: 4px 0;
}

.warning-text {
  color: #f56c6c;
  font-weight: 600;
  margin-top: 16px;
}

/* 统计信息 */
.stats-row {
  display: flex;
  justify-content: space-around;
  margin: 16px 0;
  padding: 12px;
  background: var(--nw-bg-tertiary);
  border-radius: var(--nw-radius-sm);
}

.stats-row :deep(.el-statistic__content) {
  font-size: 20px;
  font-weight: 600;
  color: var(--nw-primary);
}

.stats-row :deep(.el-statistic__title) {
  font-size: 12px;
  color: var(--nw-text-tertiary);
}

.group-progress-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--nw-text-secondary);
  margin-bottom: 8px;
}
</style>
