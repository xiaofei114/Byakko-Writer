<script setup lang="ts">
import { ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { WarningFilled, Refresh } from '@element-plus/icons-vue';
import { ElMessage, ElNotification } from 'element-plus';
import { useBookStore } from '../stores/book';
import { useConfigStore } from '../stores/config';
import type { DetectedConflict } from '../types';

const props = defineProps<{ modelValue: boolean }>();
const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>();

const bookStore = useBookStore();
const configStore = useConfigStore();
const conflicts = ref<DetectedConflict[]>([]);
const isDetecting = ref(false);

const loadConflicts = async () => {
  if (!bookStore.currentBook) return;
  try {
    conflicts.value = await invoke<DetectedConflict[]>('get_active_conflicts', {
      bookId: bookStore.currentBook.id
    });
  } catch { /* ignore */ }
};

const runDetection = async () => {
  if (!bookStore.currentBook || isDetecting.value) return;
  isDetecting.value = true;
  try {
    const result = await invoke<DetectedConflict[]>('run_conflict_detection', {
      bookId: bookStore.currentBook.id,
      config: {
        provider: configStore.aiConfig.provider,
        apiKey: configStore.aiConfig.apiKey,
        apiUrl: configStore.aiConfig.apiUrl,
        model: configStore.aiConfig.model,
        temperature: configStore.aiConfig.temperature,
        maxTokens: configStore.aiConfig.maxTokens,
        maxRounds: configStore.aiConfig.maxRounds,
      }
    });
    conflicts.value = result;
    if (result.length > 0) {
      ElNotification({
        title: '设定冲突检测',
        message: `检测到 ${result.length} 个剧情冲突，请及时查看`,
        type: 'warning',
        duration: 5000,
      });
    }
    ElMessage.success(result.length === 0 ? '未检测到设定冲突' : `检测到 ${result.length} 个冲突`);
  } catch (e) {
    ElMessage.error('检测失败: ' + e);
  } finally {
    isDetecting.value = false;
  }
};

const ignoreConflict = async (id: string) => {
  try {
    await invoke('ignore_conflict', { conflictId: id });
    conflicts.value = conflicts.value.filter(c => c.id !== id);
    ElMessage.success('已忽略');
  } catch (e) {
    ElMessage.error('操作失败: ' + e);
  }
};

const severityTag = (s?: string) => {
  if (s === 'high') return 'danger';
  if (s === 'medium') return 'warning';
  return 'info';
};
const severityLabel = (s?: string) => {
  if (s === 'high') return '严重';
  if (s === 'medium') return '中等';
  return '轻微';
};

watch(() => props.modelValue, (v) => { if (v) loadConflicts(); });
</script>

<template>
  <el-drawer
    :model-value="modelValue"
    title="设定冲突检测"
    size="400px"
    direction="rtl"
    :close-on-click-modal="false"
    @close="emit('update:modelValue', false)"
  >
    <template #header>
      <div style="display:flex;align-items:center;justify-content:space-between;width:100%;padding-right:24px;">
        <span style="display:flex;align-items:center;gap:8px;font-size:16px;font-weight:600;">
          <el-icon :size="20" color="var(--el-color-warning)"><WarningFilled /></el-icon>
          设定冲突检测
        </span>
        <el-button type="warning" size="small" :icon="Refresh" :loading="isDetecting" @click="runDetection">
          开始检测
        </el-button>
      </div>
    </template>

    <div v-if="conflicts.length === 0" class="empty-state">
      <p v-if="isDetecting">正在检测中...</p>
      <p v-else>暂无冲突，点击"开始检测"检查书籍设定一致性</p>
    </div>

    <div v-for="c in conflicts" :key="c.id" class="conflict-item">
      <div class="conflict-header">
        <el-tag :type="severityTag(c.severity)" size="small">{{ severityLabel(c.severity) }}</el-tag>
      </div>
      <div class="conflict-desc">{{ c.description }}</div>
      <div v-if="c.suggestion" class="conflict-sug">建议：{{ c.suggestion }}</div>
      <div class="conflict-actions">
        <el-button size="small" type="warning" @click="ignoreConflict(c.id)">忽略</el-button>
      </div>
    </div>
  </el-drawer>
</template>

<style scoped>
.empty-state { text-align:center;color:var(--nw-text-secondary);font-size:14px;padding:40px 0; }
.conflict-item { padding:12px;margin-bottom:10px;background:var(--nw-bg-secondary);border-radius:8px;border-left:3px solid var(--el-color-warning); }
.conflict-header { margin-bottom:6px; }
.conflict-desc { font-size:14px;line-height:1.6; }
.conflict-sug { font-size:13px;color:var(--nw-text-secondary);margin-top:6px; }
.conflict-actions { display:flex;gap:8px;margin-top:10px;justify-content:flex-end; }
</style>
