<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import { useBookStore } from '../stores/book';
import { ElMessage, ElMessageBox } from 'element-plus';
import {
  Collection,
  RefreshLeft,
  Plus,
  Calendar,
  Document,
  Warning,
  ArrowLeft,
  ArrowRight,
  View,
  InfoFilled
} from '@element-plus/icons-vue';
import type { SnapshotComparison } from '../types';

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const bookStore = useBookStore();
const loading = ref(false);
const creating = ref(false);
const snapshotName = ref('');
const showCreateForm = ref(false);
const viewMode = ref<'list' | 'detail'>('list');
const selectedChapterId = ref<string | null>(null);

// 当前章节的快照列表
const chapterSnapshots = ref<any[]>([]);

// 对比相关
const showCompareDialog = ref(false);
const compareLoading = ref(false);
const compareResult = ref<SnapshotComparison | null>(null);
const selectedSnapshots = ref<string[]>([]);

// 按章节分组的快照
const groupedSnapshots = ref<Map<string, any[]>>(new Map());

// 加载所有章节的快照
const loadAllSnapshots = async () => {
  if (!bookStore.currentBook) return;
  loading.value = true;
  
  try {
    const groups = new Map<string, any[]>();
    
    for (const chapter of bookStore.currentBook.chapters) {
      const snapshots = await bookStore.listChapterSnapshots(chapter.id);
      if (snapshots.length > 0) {
        groups.set(chapter.id, snapshots);
      }
    }
    
    groupedSnapshots.value = groups;
  } catch (error) {
    console.error('加载快照列表失败:', error);
  } finally {
    loading.value = false;
  }
};

// 加载当前章节的快照
const loadChapterSnapshots = async (chapterId: string) => {
  loading.value = true;
  try {
    chapterSnapshots.value = await bookStore.listChapterSnapshots(chapterId);
    selectedChapterId.value = chapterId;
    viewMode.value = 'detail';
  } catch (error) {
    console.error('加载章节快照失败:', error);
  } finally {
    loading.value = false;
  }
};

// 返回列表视图
const backToList = () => {
  viewMode.value = 'list';
  selectedChapterId.value = null;
  chapterSnapshots.value = [];
};

// 创建快照
const handleCreate = async () => {
  if (!snapshotName.value.trim()) {
    ElMessage.warning('请输入快照名称');
    return;
  }
  
  const chapterId = selectedChapterId.value || bookStore.currentChapterId;
  if (!chapterId) {
    ElMessage.warning('请先选择章节');
    return;
  }

  creating.value = true;
  try {
    await bookStore.createChapterSnapshot(snapshotName.value.trim(), chapterId);
    ElMessage.success('快照创建成功');
    snapshotName.value = '';
    showCreateForm.value = false;
    
    // 刷新当前视图
    if (viewMode.value === 'detail' && selectedChapterId.value) {
      await loadChapterSnapshots(selectedChapterId.value);
    } else {
      await loadAllSnapshots();
    }
  } catch (error) {
    console.error('创建快照失败:', error);
    ElMessage.error('创建快照失败');
  } finally {
    creating.value = false;
  }
};

// 恢复快照
const handleRestore = async (snapshot: any) => {
  try {
    await ElMessageBox.confirm(
      `确定要恢复到快照 "${snapshot.name}" 吗？\n当前内容将被覆盖，建议先创建当前版本的快照。`,
      '确认恢复',
      {
        confirmButtonText: '恢复',
        cancelButtonText: '取消',
        type: 'warning',
        icon: Warning
      }
    );

    const success = await bookStore.restoreChapterSnapshot(snapshot.id);
    if (success) {
      ElMessage.success('快照恢复成功');
      emit('update:modelValue', false);
      // 刷新页面内容
      window.location.reload();
    }
  } catch (error) {
    if (error !== 'cancel') {
      console.error('恢复快照失败:', error);
      ElMessage.error('恢复快照失败');
    }
  }
};

// 删除快照
const handleDelete = async (snapshot: any) => {
  try {
    await ElMessageBox.confirm(
      `确定要删除快照 "${snapshot.name}" 吗？`,
      '确认删除',
      {
        confirmButtonText: '删除',
        cancelButtonText: '取消',
        type: 'warning'
      }
    );

    await bookStore.deleteChapterSnapshot(snapshot.id);
    ElMessage.success('快照已删除');
    
    // 刷新当前视图
    if (viewMode.value === 'detail' && selectedChapterId.value) {
      await loadChapterSnapshots(selectedChapterId.value);
    } else {
      await loadAllSnapshots();
    }
  } catch (error) {
    if (error !== 'cancel') {
      console.error('删除快照失败:', error);
      ElMessage.error('删除快照失败');
    }
  }
};

// 获取章节标题
const getChapterTitle = (chapterId: string): string => {
  const chapter = bookStore.currentBook?.chapters.find(c => c.id === chapterId);
  return chapter?.title || '未知章节';
};

// 获取章节最新快照时间
const getLatestSnapshotTime = (chapterId: string): string => {
  const snapshots = groupedSnapshots.value.get(chapterId);
  if (!snapshots || snapshots.length === 0) return '';
  return formatDate(snapshots[0].created_at);
};

// 获取章节快照数量
const getSnapshotCount = (chapterId: string): number => {
  return groupedSnapshots.value.get(chapterId)?.length || 0;
};

// 格式化日期
const formatDate = (dateStr: string) => {
  const date = new Date(dateStr);
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  });
};

// 格式化字数
const formatWordCount = (count: number) => {
  if (count >= 10000) {
    return (count / 10000).toFixed(1) + '万';
  }
  return count.toString();
};

// 打开对比对话框
const openCompareDialog = () => {
  if (selectedSnapshots.value.length !== 2) {
    ElMessage.warning('请选择两个快照进行对比');
    return;
  }
  showCompareDialog.value = true;
  handleCompare();
};

// 执行对比
const handleCompare = async () => {
  if (selectedSnapshots.value.length !== 2) return;
  
  compareLoading.value = true;
  try {
    const result = await bookStore.compareSnapshots(
      selectedSnapshots.value[0],
      selectedSnapshots.value[1]
    );
    compareResult.value = result;
  } catch (error) {
    console.error('对比快照失败:', error);
    ElMessage.error('对比快照失败');
  } finally {
    compareLoading.value = false;
  }
};

// 关闭对比对话框
const closeCompareDialog = () => {
  showCompareDialog.value = false;
  compareResult.value = null;
  selectedSnapshots.value = [];
};

// 获取差异块的样式类
const getDiffBlockClass = (blockType: string) => {
  switch (blockType) {
    case 'insert':
      return 'diff-insert';
    case 'delete':
      return 'diff-delete';
    default:
      return 'diff-equal';
  }
};

// 获取差异块的标签
const getDiffBlockLabel = (blockType: string) => {
  switch (blockType) {
    case 'insert':
      return '新增';
    case 'delete':
      return '删除';
    case 'equal':
      return '未变更';
    default:
      return '';
  }
};

// 统计新增行数
const getInsertCount = (blocks: any[]) => {
  return blocks
    .filter(b => b.block_type === 'insert')
    .reduce((sum, b) => sum + b.new_text.split('\n').length, 0);
};

// 统计删除行数
const getDeleteCount = (blocks: any[]) => {
  return blocks
    .filter(b => b.block_type === 'delete')
    .reduce((sum, b) => sum + b.old_text.split('\n').length, 0);
};

// 统计未变更行数
const getEqualCount = (blocks: any[]) => {
  return blocks
    .filter(b => b.block_type === 'equal')
    .reduce((sum, b) => sum + b.new_text.split('\n').length, 0);
};

// 切换快照选择状态
const toggleSnapshotSelection = (snapshotId: string) => {
  const index = selectedSnapshots.value.indexOf(snapshotId);
  if (index > -1) {
    // 已选中，取消选择
    selectedSnapshots.value.splice(index, 1);
  } else if (selectedSnapshots.value.length < 2) {
    // 未选中且未满2个，添加选择
    selectedSnapshots.value.push(snapshotId);
  } else {
    // 已满2个，替换第二个
    selectedSnapshots.value[1] = snapshotId;
  }
};

// 清除选择
const clearSelection = () => {
  selectedSnapshots.value = [];
};

// 获取选择状态样式类
const getSelectionClass = (snapshotId: string) => {
  if (selectedSnapshots.value[0] === snapshotId) return 'is-old';
  if (selectedSnapshots.value[1] === snapshotId) return 'is-new';
  return '';
};

onMounted(() => {
  loadAllSnapshots();
});

// 监听对话框打开
watch(() => props.modelValue, (open) => {
  if (open) {
    loadAllSnapshots();
    viewMode.value = 'list';
    selectedChapterId.value = null;
    selectedSnapshots.value = [];
  }
});
</script>

<template>
  <el-drawer
    :model-value="modelValue"
    @update:model-value="$emit('update:modelValue', $event)"
    :title="viewMode === 'detail' ? `${getChapterTitle(selectedChapterId!)} - 版本快照` : '版本快照'"
    direction="ltr"
    size="400px"
    :close-on-click-modal="false"
    class="snapshot-drawer"
  >
    <div class="snapshot-manager">
      <!-- 头部操作区 -->
      <div class="snapshot-header">
        <div class="header-left">
          <el-button
            v-if="viewMode === 'detail'"
            text
            :icon="ArrowLeft"
            @click="backToList"
          >
            返回
          </el-button>
        </div>
        <div class="header-right">
          <el-button
            type="primary"
            :icon="Plus"
            @click="showCreateForm = true"
            :disabled="showCreateForm"
          >
            {{ viewMode === 'detail' ? '创建快照' : '为当前章节创建' }}
          </el-button>
          <el-button
            text
            :icon="RefreshLeft"
            @click="viewMode === 'detail' ? loadChapterSnapshots(selectedChapterId!) : loadAllSnapshots()"
            :loading="loading"
          >
            刷新
          </el-button>
        </div>
      </div>

      <!-- 创建表单 -->
      <el-collapse-transition>
        <div v-if="showCreateForm" class="create-form">
          <el-input
            v-model="snapshotName"
            placeholder="请输入快照名称（如：修订版、备份等）"
            maxlength="50"
            show-word-limit
            @keyup.enter="handleCreate"
          />
          <div class="create-form-actions">
            <el-button
              type="primary"
              @click="handleCreate"
              :loading="creating"
            >
              创建
            </el-button>
            <el-button @click="showCreateForm = false">
              取消
            </el-button>
          </div>
        </div>
      </el-collapse-transition>

      <!-- 章节列表视图 -->
      <div v-if="viewMode === 'list'" v-loading="loading" class="chapter-list">
        <el-empty v-if="groupedSnapshots.size === 0" description="暂无快照，点击上方按钮为当前章节创建" />
        
        <div
          v-for="[chapterId] in groupedSnapshots"
          :key="chapterId"
          class="chapter-item"
          @click="loadChapterSnapshots(chapterId)"
        >
          <div class="chapter-info">
            <div class="chapter-title">
              <el-icon><Collection /></el-icon>
              <span>{{ getChapterTitle(chapterId) }}</span>
            </div>
            <div class="chapter-meta">
              <span class="meta-item">
                <el-icon><Calendar /></el-icon>
                最新: {{ getLatestSnapshotTime(chapterId) }}
              </span>
              <span class="meta-item">
                <el-icon><Document /></el-icon>
                {{ getSnapshotCount(chapterId) }} 个版本
              </span>
            </div>
          </div>
          <el-icon class="arrow-icon"><ArrowRight /></el-icon>
        </div>
      </div>

      <!-- 章节快照详情视图 -->
      <div v-else v-loading="loading" class="snapshot-list">
        <el-empty v-if="chapterSnapshots.length === 0" description="该章节暂无快照" />
        
        <!-- 对比操作栏 -->
        <div v-if="chapterSnapshots.length >= 2 && selectedSnapshots.length === 2" class="compare-bar">
          <span class="compare-hint">已选择两个版本</span>
          <el-button
            type="primary"
            size="small"
            :icon="View"
            @click="openCompareDialog"
          >
            对比选中版本
          </el-button>
          <el-button
            text
            size="small"
            @click="clearSelection"
          >
            清除选择
          </el-button>
        </div>
        <div v-else-if="chapterSnapshots.length >= 2" class="compare-hint-bar">
          <el-icon><InfoFilled /></el-icon>
          <span>点击快照卡片选择两个版本进行对比</span>
        </div>
        
        <div
          v-for="(snapshot, index) in chapterSnapshots"
          :key="snapshot.id"
          class="snapshot-item"
          :class="{ 
            'is-selected': selectedSnapshots.includes(snapshot.id),
            'is-old': selectedSnapshots[0] === snapshot.id,
            'is-new': selectedSnapshots[1] === snapshot.id
          }"
          @click="toggleSnapshotSelection(snapshot.id)"
        >
          <!-- 选择指示器 -->
          <div class="snapshot-selection">
            <div 
              class="selection-indicator" 
              :class="getSelectionClass(snapshot.id)"
            >
              <template v-if="selectedSnapshots[0] === snapshot.id">
                <span class="selection-text">旧</span>
              </template>
              <template v-else-if="selectedSnapshots[1] === snapshot.id">
                <span class="selection-text">新</span>
              </template>
              <template v-else>
                <span class="selection-order">{{ index + 1 }}</span>
              </template>
            </div>
          </div>
          
          <!-- 快照信息 -->
          <div class="snapshot-content">
            <div class="snapshot-header-row">
              <div class="snapshot-title-wrap">
                <el-icon class="snapshot-icon"><Collection /></el-icon>
                <span class="snapshot-title-text">{{ snapshot.name }}</span>
              </div>
              <div class="snapshot-badges">
                <el-tag v-if="selectedSnapshots[0] === snapshot.id" size="small" type="info" effect="light">旧版本</el-tag>
                <el-tag v-else-if="selectedSnapshots[1] === snapshot.id" size="small" type="success" effect="light">新版本</el-tag>
              </div>
            </div>
            <div class="snapshot-meta-row">
              <span class="meta-time">
                <el-icon><Calendar /></el-icon>
                {{ formatDate(snapshot.created_at) }}
              </span>
              <span class="meta-divider">·</span>
              <span class="meta-words">{{ formatWordCount(snapshot.word_count) }} 字</span>
            </div>
          </div>
          
          <!-- 操作按钮 -->
          <div class="snapshot-actions" @click.stop>
            <el-button
              text
              type="primary"
              size="small"
              @click="handleRestore(snapshot)"
            >
              恢复
            </el-button>
            <el-button
              text
              type="danger"
              size="small"
              @click="handleDelete(snapshot)"
            >
              删除
            </el-button>
          </div>
        </div>
      </div>

      <!-- 提示信息 -->
      <div class="snapshot-tips">
        <el-alert
          title="使用提示"
          type="info"
          :closable="false"
          show-icon
        >
          <template #default>
            <ul>
              <li>每个章节的快照是独立的</li>
              <li>自动保存的快照会保留最近20个版本</li>
              <li>恢复快照会覆盖当前章节内容，请谨慎操作</li>
              <li>选择两个快照可以对比版本差异</li>
            </ul>
          </template>
        </el-alert>
      </div>
    </div>
  </el-drawer>

  <!-- 对比对话框 -->
  <el-dialog
    v-model="showCompareDialog"
    title="版本对比"
    width="85%"
    :close-on-click-modal="false"
    class="compare-dialog"
    destroy-on-close
  >
    <div v-loading="compareLoading" class="compare-content">
      <!-- 对比头部信息 -->
      <div v-if="compareResult" class="compare-header">
        <div class="compare-card old">
          <div class="compare-card-label">
            <el-icon><ArrowLeft /></el-icon>
            <span>旧版本</span>
          </div>
          <div class="compare-card-title">{{ compareResult.old_snapshot.name }}</div>
          <div class="compare-card-meta">
            <el-icon><Calendar /></el-icon>
            <span>{{ formatDate(compareResult.old_snapshot.created_at) }}</span>
            <span class="meta-sep">·</span>
            <span>{{ formatWordCount(compareResult.old_snapshot.word_count) }} 字</span>
          </div>
        </div>
        
        <div class="compare-vs">
          <div class="vs-line"></div>
          <span class="vs-text">VS</span>
          <div class="vs-line"></div>
        </div>
        
        <div class="compare-card new">
          <div class="compare-card-label">
            <el-icon><ArrowRight /></el-icon>
            <span>新版本</span>
          </div>
          <div class="compare-card-title">{{ compareResult.new_snapshot.name }}</div>
          <div class="compare-card-meta">
            <el-icon><Calendar /></el-icon>
            <span>{{ formatDate(compareResult.new_snapshot.created_at) }}</span>
            <span class="meta-sep">·</span>
            <span>{{ formatWordCount(compareResult.new_snapshot.word_count) }} 字</span>
          </div>
        </div>
      </div>
      
      <!-- 统计信息 -->
      <div v-if="compareResult" class="diff-stats">
        <div class="stat-item insert">
          <span class="stat-label">新增</span>
          <span class="stat-value">{{ getInsertCount(compareResult.diff_blocks) }} 行</span>
        </div>
        <div class="stat-item delete">
          <span class="stat-label">删除</span>
          <span class="stat-value">{{ getDeleteCount(compareResult.diff_blocks) }} 行</span>
        </div>
        <div class="stat-item equal">
          <span class="stat-label">未变更</span>
          <span class="stat-value">{{ getEqualCount(compareResult.diff_blocks) }} 行</span>
        </div>
      </div>
      
      <!-- 差异内容 -->
      <div v-if="compareResult" class="diff-container">
        <div
          v-for="(block, index) in compareResult.diff_blocks"
          :key="index"
          class="diff-block"
          :class="getDiffBlockClass(block.block_type)"
        >
          <div class="diff-block-header">
            <span class="diff-type-badge">{{ getDiffBlockLabel(block.block_type) }}</span>
          </div>
          <pre class="diff-text">{{ block.block_type === 'delete' ? block.old_text : block.new_text }}</pre>
        </div>
      </div>
      
      <el-empty v-else-if="!compareLoading" description="无法加载对比结果" />
    </div>
    
    <template #footer>
      <el-button @click="closeCompareDialog">关闭</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
/* 抽屉样式 */
.snapshot-drawer :deep(.el-drawer__body) {
  padding: 16px;
}

.snapshot-manager {
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;
}

.snapshot-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.header-left,
.header-right {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.create-form {
  padding: 12px;
  background: var(--el-fill-color-light);
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.create-form-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.chapter-list,
.snapshot-list {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.chapter-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px;
  border: 1px solid var(--el-border-color-light);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.chapter-item:hover {
  background: var(--el-fill-color-light);
  border-color: var(--el-color-primary-light-5);
}

.chapter-info {
  flex: 1;
  min-width: 0;
}

.chapter-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 500;
  font-size: 14px;
  margin-bottom: 4px;
}

.chapter-title .el-icon {
  color: var(--el-color-primary);
}

.chapter-meta {
  display: flex;
  gap: 16px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.arrow-icon {
  color: var(--el-text-color-secondary);
}

.snapshot-item {
  display: flex;
  align-items: flex-start;
  padding: 12px 14px;
  border: 1px solid var(--el-border-color-light);
  border-radius: 10px;
  transition: all 0.2s ease;
  gap: 12px;
}

.snapshot-item:hover {
  background: var(--el-fill-color-light);
  border-color: var(--el-color-primary-light-5);
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
}

/* 选择指示器 */
.snapshot-selection {
  flex-shrink: 0;
  padding-top: 2px;
}

.selection-indicator {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 600;
  border: 2px solid var(--el-border-color);
  color: var(--el-text-color-placeholder);
  background: var(--el-bg-color);
  transition: all 0.2s ease;
}

.selection-indicator.is-old {
  border-color: var(--el-color-info);
  background: var(--el-color-info);
  color: white;
}

.selection-indicator.is-new {
  border-color: var(--el-color-success);
  background: var(--el-color-success);
  color: white;
}

.selection-text {
  font-size: 10px;
}

.selection-order {
  font-size: 11px;
}

/* 快照内容区 */
.snapshot-content {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.snapshot-header-row {
  display: flex;
  align-items: center;
  gap: 8px;
  justify-content: space-between;
}

.snapshot-title-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
}

.snapshot-icon {
  color: var(--el-color-primary);
  font-size: 16px;
  flex-shrink: 0;
}

.snapshot-title-text {
  font-weight: 500;
  font-size: 14px;
  color: var(--el-text-color-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.snapshot-badges {
  flex-shrink: 0;
}

.snapshot-meta-row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.meta-time {
  display: flex;
  align-items: center;
  gap: 4px;
}

.meta-time .el-icon {
  font-size: 12px;
}

.meta-divider {
  color: var(--el-text-color-placeholder);
}

.meta-words {
  color: var(--el-text-color-regular);
}

/* 操作按钮 */
.snapshot-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
  padding-top: 2px;
}

.snapshot-actions .el-button {
  padding: 4px 10px;
}

.snapshot-tips {
  margin-top: 8px;
}

.snapshot-tips ul {
  margin: 0;
  padding-left: 16px;
}

.snapshot-tips li {
  margin: 4px 0;
}

/* 提示栏 */
.compare-hint-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  background: var(--el-fill-color-light);
  border-radius: 10px;
  font-size: 13px;
  color: var(--el-text-color-secondary);
  border: 1px dashed var(--el-border-color);
}

.compare-hint-bar .el-icon {
  color: var(--el-color-primary);
}

/* 对比操作栏 */
.compare-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 16px;
  background: linear-gradient(135deg, var(--el-color-primary-light-9) 0%, var(--el-color-primary-light-8) 100%);
  border-radius: 10px;
  border: 1px solid var(--el-color-primary-light-5);
}

.compare-hint {
  flex: 1;
  font-size: 14px;
  font-weight: 500;
  color: var(--el-color-primary);
}

.compare-bar .el-button {
  font-weight: 500;
}

.snapshot-item {
  cursor: pointer;
  position: relative;
}

.snapshot-item:hover {
  border-color: var(--el-color-primary-light-5);
}

.snapshot-item.is-selected {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
  box-shadow: 0 0 0 2px var(--el-color-primary-light-7);
}

.snapshot-item.is-old {
  border-color: var(--el-color-info);
  background: var(--el-color-info-light-9);
  box-shadow: 0 0 0 2px var(--el-color-info-light-7);
}

.snapshot-item.is-old .snapshot-icon {
  color: var(--el-color-info);
}

.snapshot-item.is-new {
  border-color: var(--el-color-success);
  background: var(--el-color-success-light-9);
  box-shadow: 0 0 0 2px var(--el-color-success-light-7);
}

.snapshot-item.is-new .snapshot-icon {
  color: var(--el-color-success);
}

/* 选择指示器 */
.snapshot-selection {
  display: flex;
  align-items: center;
  margin-right: 12px;
}

.selection-indicator {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 500;
  border: 2px solid var(--el-border-color);
  color: var(--el-text-color-secondary);
  background: var(--el-bg-color);
  transition: all 0.2s;
}

.selection-indicator.is-old {
  border-color: var(--el-color-info);
  background: var(--el-color-info);
  color: white;
}

.selection-indicator.is-new {
  border-color: var(--el-color-success);
  background: var(--el-color-success);
  color: white;
}

.selection-number {
  font-size: 12px;
}

/* 对比对话框样式 */
.compare-dialog :deep(.el-dialog__body) {
  padding: 0;
  max-height: 70vh;
  overflow: hidden;
}

.compare-content {
  display: flex;
  flex-direction: column;
  height: 70vh;
}

/* 对比头部 */
.compare-header {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 16px;
  padding: 20px 24px;
  background: var(--el-fill-color-light);
  border-bottom: 1px solid var(--el-border-color-light);
}

.compare-card {
  flex: 1;
  max-width: 280px;
  padding: 16px 20px;
  background: var(--el-bg-color);
  border-radius: 10px;
  border: 2px solid var(--el-border-color);
  text-align: center;
}

.compare-card.old {
  border-color: var(--el-color-info-light-5);
}

.compare-card.new {
  border-color: var(--el-color-success-light-5);
}

.compare-card-label {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
  margin-bottom: 8px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.compare-card.old .compare-card-label {
  color: var(--el-color-info);
}

.compare-card.new .compare-card-label {
  color: var(--el-color-success);
}

.compare-card-title {
  font-weight: 600;
  font-size: 15px;
  color: var(--el-text-color-primary);
  margin-bottom: 8px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.compare-card-meta {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.compare-card-meta .el-icon {
  font-size: 12px;
}

.meta-sep {
  color: var(--el-text-color-placeholder);
}

/* VS 分隔 */
.compare-vs {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}

.vs-line {
  width: 1px;
  height: 20px;
  background: var(--el-border-color);
}

.vs-text {
  font-size: 11px;
  font-weight: 600;
  color: var(--el-text-color-placeholder);
  padding: 4px 8px;
  border: 1px solid var(--el-border-color);
  border-radius: 12px;
}

/* 统计信息 */
.diff-stats {
  display: flex;
  justify-content: center;
  gap: 24px;
  padding: 12px 20px;
  background: var(--el-bg-color);
  border-bottom: 1px solid var(--el-border-color-light);
}

.stat-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
}

.stat-label {
  font-size: 11px;
  color: var(--el-text-color-secondary);
}

.stat-value {
  font-size: 14px;
  font-weight: 600;
}

.stat-item.insert .stat-value {
  color: var(--el-color-success);
}

.stat-item.delete .stat-value {
  color: var(--el-color-danger);
}

.stat-item.equal .stat-value {
  color: var(--el-text-color-regular);
}

/* 差异内容 */
.diff-container {
  flex: 1;
  overflow-y: auto;
  padding: 16px 24px;
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 13px;
  line-height: 1.6;
  background: var(--el-bg-color);
}

.diff-block {
  margin-bottom: 12px;
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--el-border-color-light);
}

.diff-block-header {
  display: flex;
  align-items: center;
  padding: 6px 12px;
  font-size: 11px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.diff-type-badge {
  padding: 2px 8px;
  border-radius: 4px;
}

.diff-block.diff-equal {
  border-color: var(--el-border-color-light);
}

.diff-block.diff-equal .diff-block-header {
  background: var(--el-fill-color-light);
  color: var(--el-text-color-secondary);
}

.diff-block.diff-insert {
  border-color: var(--el-color-success-light-3);
}

.diff-block.diff-insert .diff-block-header {
  background: var(--el-color-success-light-8);
  color: var(--el-color-success);
}

.diff-block.diff-insert .diff-type-badge {
  background: var(--el-color-success);
  color: white;
}

.diff-block.diff-delete {
  border-color: var(--el-color-danger-light-3);
}

.diff-block.diff-delete .diff-block-header {
  background: var(--el-color-danger-light-8);
  color: var(--el-color-danger);
}

.diff-block.diff-delete .diff-type-badge {
  background: var(--el-color-danger);
  color: white;
}

.diff-text {
  margin: 0;
  padding: 12px 16px;
  white-space: pre-wrap;
  word-wrap: break-word;
  font-family: inherit;
  font-size: 13px;
  line-height: 1.7;
  color: var(--el-text-color-primary);
}
</style>
