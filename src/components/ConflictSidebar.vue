<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { WarningFilled, Refresh, Delete, Check, Close, Tools, ArrowDown } from '@element-plus/icons-vue';
import { ElMessage, ElNotification, ElMessageBox } from 'element-plus';
import { useBookStore } from '../stores/book';
import { useConfigStore } from '../stores/config';
import type { DetectedConflict } from '../types';

const props = defineProps<{ modelValue: boolean }>();
const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void; (e: 'fixConflict', conflict: DetectedConflict): void }>();

const bookStore = useBookStore();
const configStore = useConfigStore();
const conflicts = ref<DetectedConflict[]>([]);
const isDetecting = ref(false);
const showIgnored = ref(false);

// 计算属性：未忽略的冲突
const activeConflicts = computed(() => conflicts.value.filter(c => c.isIgnored !== 1));
// 计算属性：已忽略的冲突
const ignoredConflicts = computed(() => conflicts.value.filter(c => c.isIgnored === 1));

const loadConflicts = async () => {
  if (!bookStore.currentBook) return;
  try {
    conflicts.value = await invoke<DetectedConflict[]>('get_all_conflicts', {
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
    await loadConflicts();
    const activeCount = result.filter(c => c.isIgnored !== 1).length;
    if (activeCount > 0) {
      ElNotification({
        title: '设定冲突检测',
        message: `检测到 ${activeCount} 个新的剧情冲突，请及时查看`,
        type: 'warning',
        duration: 5000,
      });
    }
    ElMessage.success(activeCount === 0 ? '未检测到设定冲突' : `检测到 ${activeCount} 个新冲突`);
  } catch (e) {
    ElMessage.error('检测失败: ' + e);
  } finally {
    isDetecting.value = false;
  }
};

const ignoreConflict = async (id: string) => {
  try {
    await invoke('ignore_conflict', { conflictId: id });
    await loadConflicts();
    ElMessage.success('已忽略');
  } catch (e) {
    ElMessage.error('操作失败: ' + e);
  }
};

const unignoreConflict = async (id: string) => {
  try {
    await invoke('unignore_conflict', { conflictId: id });
    await loadConflicts();
    ElMessage.success('已取消忽略');
  } catch (e) {
    ElMessage.error('操作失败: ' + e);
  }
};

const deleteConflict = async (id: string) => {
  try {
    await ElMessageBox.confirm('确定要删除这个冲突记录吗？', '确认删除', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    });
    await invoke('delete_conflict', { conflictId: id });
    await loadConflicts();
    ElMessage.success('已删除');
  } catch (e: any) {
    if (e !== 'cancel') {
      ElMessage.error('删除失败: ' + e);
    }
  }
};

const fixConflict = (conflict: DetectedConflict) => {
  emit('fixConflict', conflict);
  emit('update:modelValue', false);
  ElMessage.info('已将冲突发送到AI会话');
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
    size="450px"
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

    <!-- 未忽略的冲突 -->
    <div v-if="activeConflicts.length === 0 && ignoredConflicts.length === 0" class="empty-state">
      <p v-if="isDetecting">正在检测中...</p>
      <p v-else>暂无冲突，点击"开始检测"检查书籍设定一致性</p>
    </div>

    <template v-if="activeConflicts.length > 0">
      <div class="section-title">
        <span>待处理冲突 ({{ activeConflicts.length }})</span>
      </div>
      <div v-for="c in activeConflicts" :key="c.id" class="conflict-item">
        <div class="conflict-header">
          <el-tag :type="severityTag(c.severity)" size="small">{{ severityLabel(c.severity) }}</el-tag>
          <el-tag v-if="c.isIgnored === 1" type="info" size="small">已忽略</el-tag>
        </div>
        <div class="conflict-desc">{{ c.description }}</div>
        <div v-if="c.suggestion" class="conflict-sug">建议：{{ c.suggestion }}</div>
        <div class="conflict-actions">
          <el-button size="small" type="primary" :icon="Tools" @click="fixConflict(c)">修复</el-button>
          <el-button size="small" type="warning" :icon="Check" @click="ignoreConflict(c.id)">忽略</el-button>
          <el-button size="small" type="danger" :icon="Delete" @click="deleteConflict(c.id)">删除</el-button>
        </div>
      </div>
    </template>

    <!-- 已忽略的冲突 -->
    <template v-if="ignoredConflicts.length > 0">
      <div class="section-title ignored-title" @click="showIgnored = !showIgnored">
        <span>已忽略的冲突 ({{ ignoredConflicts.length }})</span>
        <el-icon class="toggle-icon" :class="{ 'is-open': showIgnored }"><arrow-down /></el-icon>
      </div>
      <div v-show="showIgnored" class="ignored-section">
        <div v-for="c in ignoredConflicts" :key="c.id" class="conflict-item ignored-item">
          <div class="conflict-header">
            <el-tag :type="severityTag(c.severity)" size="small">{{ severityLabel(c.severity) }}</el-tag>
            <el-tag type="info" size="small">已忽略</el-tag>
          </div>
          <div class="conflict-desc">{{ c.description }}</div>
          <div v-if="c.suggestion" class="conflict-sug">建议：{{ c.suggestion }}</div>
          <div class="conflict-actions">
            <el-button size="small" type="primary" :icon="Tools" @click="fixConflict(c)">修复</el-button>
            <el-button size="small" :icon="Close" @click="unignoreConflict(c.id)">取消忽略</el-button>
            <el-button size="small" type="danger" :icon="Delete" @click="deleteConflict(c.id)">删除</el-button>
          </div>
        </div>
      </div>
    </template>
  </el-drawer>
</template>

<style scoped>
.empty-state { text-align:center;color:var(--nw-text-secondary);font-size:14px;padding:40px 0; }

.section-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 14px;
  font-weight: 600;
  color: var(--el-color-warning);
  margin: 16px 0 12px 0;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--el-border-color-light);
}

.section-title.ignored-title {
  color: var(--el-text-color-secondary);
  cursor: pointer;
  user-select: none;
}

.section-title.ignored-title:hover {
  color: var(--el-text-color-regular);
}

.toggle-icon {
  transition: transform 0.3s;
}

.toggle-icon.is-open {
  transform: rotate(180deg);
}

.ignored-section {
  opacity: 0.7;
}

.conflict-item {
  padding: 12px;
  margin-bottom: 10px;
  background: var(--nw-bg-secondary);
  border-radius: 8px;
  border-left: 3px solid var(--el-color-warning);
}

.conflict-item.ignored-item {
  border-left-color: var(--el-text-color-disabled);
  background: var(--el-fill-color-light);
}

.conflict-header {
  display: flex;
  gap: 8px;
  margin-bottom: 6px;
}

.conflict-desc {
  font-size: 14px;
  line-height: 1.6;
}

.conflict-sug {
  font-size: 13px;
  color: var(--nw-text-secondary);
  margin-top: 6px;
}

.conflict-actions {
  display: flex;
  gap: 8px;
  margin-top: 10px;
  flex-wrap: wrap;
}
</style>
