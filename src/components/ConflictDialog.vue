<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { WarningFilled } from '@element-plus/icons-vue';
import { ElMessage } from 'element-plus';
import type { DetectedConflict } from '../types';

defineProps<{
  visible: boolean;
  conflicts: DetectedConflict[];
  bookId: string;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'ignored', conflictId: string): void;
}>();

const handleIgnore = async (conflict: DetectedConflict) => {
  try {
    await invoke('ignore_conflict', { conflictId: conflict.id });
    emit('ignored', conflict.id);
    ElMessage.success('已忽略该冲突');
  } catch (e) {
    ElMessage.error('操作失败: ' + e);
  }
};

const handleClose = () => {
  emit('close');
};

const severityTag = (severity?: string) => {
  switch (severity) {
    case 'high': return 'danger';
    case 'medium': return 'warning';
    case 'low': return 'info';
    default: return 'info';
  }
};

const severityLabel = (severity?: string) => {
  switch (severity) {
    case 'high': return '严重';
    case 'medium': return '中等';
    case 'low': return '轻微';
    default: return '未知';
  }
};
</script>

<template>
  <el-dialog
    :model-value="visible"
    title="设定冲突检测"
    width="560px"
    :close-on-click-modal="false"
    @close="handleClose"
  >
    <template #header>
      <div style="display:flex;align-items:center;gap:8px;">
        <el-icon :size="20" color="var(--el-color-warning)"><WarningFilled /></el-icon>
        <span>检测到 {{ conflicts.length }} 个设定冲突</span>
      </div>
    </template>

    <div v-if="conflicts.length === 0" style="text-align:center;padding:20px;color:var(--nw-text-secondary);">
      没有未处理的冲突
    </div>

    <div v-for="conflict in conflicts" :key="conflict.id" class="conflict-item">
      <div class="conflict-header">
        <el-tag :type="severityTag(conflict.severity)" size="small">
          {{ severityLabel(conflict.severity) }}
        </el-tag>
      </div>
      <div class="conflict-desc">{{ conflict.description }}</div>
      <div v-if="conflict.suggestion" class="conflict-suggestion">
        建议：{{ conflict.suggestion }}
      </div>
      <div class="conflict-actions">
        <el-button size="small" @click="handleIgnore(conflict)">忽略</el-button>
      </div>
    </div>

    <template #footer>
      <el-button @click="handleClose">关闭</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.conflict-item {
  padding: 12px;
  margin-bottom: 12px;
  background: var(--nw-bg-secondary);
  border-radius: 8px;
  border-left: 3px solid var(--el-color-warning);
}
.conflict-header { margin-bottom: 8px; }
.conflict-desc { font-size: 14px; line-height: 1.6; margin-bottom: 6px; }
.conflict-suggestion { font-size: 13px; color: var(--nw-text-secondary); margin-bottom: 8px; }
.conflict-actions { display: flex; justify-content: flex-end; }
</style>
