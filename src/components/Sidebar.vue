<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import { useBookStore } from '../stores/book';
import {
  ArrowLeft,
  FolderAdd,
  Plus,
  Delete,
  Document,
  Folder,
  Edit,
  Collection,
  Reading,
  ArrowRight,
} from '@element-plus/icons-vue';
import { ElMessageBox, ElMessage } from 'element-plus';
import type Node from 'element-plus/es/components/tree/src/model/node';

const bookStore = useBookStore();

const props = withDefaults(defineProps<{
  showOutlineEditing?: boolean;
}>(), {
  showOutlineEditing: false,
});

const emit = defineEmits<{
  (e: 'go-home'): void;
  (e: 'show-snapshots'): void;
  (e: 'select-outline', level: 'book' | 'volume' | 'chapter', targetId?: string): void;
  (e: 'select-chapter', chapterId: string): void;
}>();

// 侧边栏模式：'chapters' | 'outline'
const sidebarMode = ref<'chapters' | 'outline'>('chapters');

// 切换侧边栏模式
const switchMode = (mode: 'chapters' | 'outline') => {
  sidebarMode.value = mode;
};

// 当外部关闭大纲编辑时，自动切回章节模式
watch(() => props.showOutlineEditing, (val) => {
  if (!val && sidebarMode.value === 'outline') {
    sidebarMode.value = 'chapters';
  }
});

// 树形数据
interface TreeNode {
  id: string;
  label: string;
  type: 'volume' | 'chapter';
  children?: TreeNode[];
  order: number;
}

// 构建树形数据
const treeData = computed<TreeNode[]>(() => {
  return bookStore.volumes.map(volume => ({
    id: volume.id,
    label: volume.title,
    type: 'volume',
    order: volume.order,
    children: bookStore.getChaptersByVolume(volume.id)
      .sort((a, b) => a.order - b.order)
      .map(chapter => ({
        id: chapter.id,
        label: chapter.title,
        type: 'chapter',
        order: chapter.order,
      })),
  }));
});

// 默认展开的节点
const defaultExpandedKeys = ref<string[]>([]);

// 监听卷展开状态变化
watch(() => bookStore.volumes, (volumes) => {
  defaultExpandedKeys.value = volumes
    .filter(v => !v.isCollapsed)
    .map(v => v.id);
}, { immediate: true, deep: true });

// 当前选中的章节
const currentChapterId = computed(() => bookStore.currentChapterId);

// 处理节点点击
const handleNodeClick = (data: TreeNode, node: Node) => {
  // 大纲模式：发送大纲选择事件
  if (sidebarMode.value === 'outline') {
    emit('select-outline', data.type, data.id);
    return;
  }
  
  // 章节模式原有逻辑
  // 如果正在编辑，先取消编辑
  if (editingNodeId.value && editingNodeId.value !== data.id) {
    cancelEdit();
    return;
  }
  
  if (data.type === 'chapter') {
    bookStore.selectChapter(data.id);
    emit('select-chapter', data.id);
  } else if (data.type === 'volume') {
    // 点击卷时切换展开/折叠
    node.expanded = !node.expanded;
    bookStore.toggleVolumeCollapse(data.id);
  }
};

// 处理节点展开/折叠
const handleNodeExpand = (data: TreeNode) => {
  if (data.type === 'volume') {
    const volume = bookStore.volumes.find(v => v.id === data.id);
    if (volume && volume.isCollapsed) {
      bookStore.toggleVolumeCollapse(data.id);
    }
  }
};

const handleNodeCollapse = (data: TreeNode) => {
  if (data.type === 'volume') {
    const volume = bookStore.volumes.find(v => v.id === data.id);
    if (volume && !volume.isCollapsed) {
      bookStore.toggleVolumeCollapse(data.id);
    }
  }
};

// 解析标题，分离序号和名称
const parseTitle = (title: string, type: 'volume' | 'chapter') => {
  const unit = type === 'volume' ? '卷' : '章';
  const match = title.match(new RegExp(`^第\\s*(\\d+)\\s*${unit}\\s*(.*)$`));
  if (match) {
    return { number: match[1], name: match[2].trim() };
  }
  // 尝试匹配纯数字开头
  const numMatch = title.match(/^(\d+)\s*[\.、\s]\s*(.*)$/);
  if (numMatch) {
    return { number: numMatch[1], name: numMatch[2].trim() };
  }
  return { number: '', name: title };
};

// 组合标题
const composeTitle = (number: string, name: string, type: 'volume' | 'chapter') => {
  const unit = type === 'volume' ? '卷' : '章';
  const num = number.trim();
  const n = name.trim();
  if (num && n) {
    return `第${num}${unit} ${n}`;
  } else if (num) {
    return `第${num}${unit}`;
  } else if (n) {
    return n;
  }
  return '';
};

// 行内编辑状态
const editingNodeId = ref<string | null>(null);
const editingNodeType = ref<'volume' | 'chapter'>('chapter');
const editingNumber = ref('');
const editingName = ref('');
const numberInputRef = ref<HTMLInputElement>();
const nameInputRef = ref<HTMLInputElement>();

const startEditing = (data: TreeNode) => {
  editingNodeId.value = data.id;
  editingNodeType.value = data.type;
  const parsed = parseTitle(data.label, data.type);
  editingNumber.value = parsed.number;
  editingName.value = parsed.name;
  
  nextTick(() => {
    numberInputRef.value?.focus();
    numberInputRef.value?.select();
  });
};

const confirmEdit = () => {
  if (!editingNodeId.value) return;
  
  const newTitle = composeTitle(editingNumber.value, editingName.value, editingNodeType.value);
  if (!newTitle) {
    editingNodeId.value = null;
    return;
  }
  
  // 找到对应的节点
  const volume = bookStore.volumes.find(v => v.id === editingNodeId.value);
  if (volume) {
    bookStore.updateVolumeTitle(editingNodeId.value, newTitle);
  } else {
    // 可能是章节
    const chapter = bookStore.chapters.find(c => c.id === editingNodeId.value);
    if (chapter) {
      bookStore.updateChapterTitle(editingNodeId.value, newTitle);
    }
  }
  
  editingNodeId.value = null;
};

const cancelEdit = () => {
  editingNodeId.value = null;
  editingNumber.value = '';
  editingName.value = '';
};

// 创建章节
const handleCreateChapter = async (volumeId?: string) => {
  if (!volumeId) {
    // 如果没有指定卷，使用第一个卷
    volumeId = bookStore.volumes[0]?.id;
    if (!volumeId) {
      ElMessage.warning('请先创建卷');
      return;
    }
  }
  
  const chapters = bookStore.getChaptersByVolume(volumeId);
  const newOrder = chapters.length;
  const newTitle = `第${newOrder + 1}章`;
  await bookStore.createChapter(newTitle, volumeId);
};

// 新建卷对话框状态
const newVolumeDialogVisible = ref(false);
const newVolumeNumber = ref('');
const newVolumeName = ref('');
const newVolumeNumberRef = ref<HTMLInputElement>();

const openNewVolumeDialog = () => {
  // 自动计算卷号
  newVolumeNumber.value = String(bookStore.volumes.length + 1);
  newVolumeName.value = '';
  newVolumeDialogVisible.value = true;
  
  nextTick(() => {
    newVolumeNumberRef.value?.focus();
    newVolumeNumberRef.value?.select();
  });
};

const confirmNewVolume = async () => {
  const title = composeTitle(newVolumeNumber.value, newVolumeName.value, 'volume');
  if (!title) {
    ElMessage.warning('请输入卷名');
    return;
  }
  
  const order = bookStore.volumes.length;
  await bookStore.createVolume(title, order);
  newVolumeDialogVisible.value = false;
  ElMessage.success('卷创建成功');
};

const cancelNewVolume = () => {
  newVolumeDialogVisible.value = false;
  newVolumeNumber.value = '';
  newVolumeName.value = '';
};

// 删除章节
const handleDeleteChapter = async (chapterId: string) => {
  try {
    await ElMessageBox.confirm('确定要删除这个章节吗？', '确认删除', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    });
    bookStore.deleteChapter(chapterId);
  } catch {
    // 用户取消
  }
};

// 删除卷
const handleDeleteVolume = async (volumeId: string) => {
  try {
    await ElMessageBox.confirm('确定要删除这个卷吗？其中的章节也会被删除。', '确认删除', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    });
    bookStore.deleteVolume(volumeId);
  } catch {
    // 用户取消
  }
};



// 返回主页
const goHome = () => {
  emit('go-home');
};

// 点击侧边栏其他区域时保存编辑
const handleSidebarClick = (event: MouseEvent) => {
  const target = event.target as HTMLElement;
  // 如果点击的不是输入框，则保存编辑
  if (editingNodeId.value && !target.closest('.inline-edit-wrapper')) {
    confirmEdit();
  }
};
</script>

<template>
  <div class="sidebar" @click="handleSidebarClick">
    <!-- 头部：返回按钮和书名 -->
    <header class="sidebar-header">
      <el-button
        circle
        :icon="ArrowLeft"
        @click="goHome"
        title="返回主页"
        class="back-btn"
      />
      <el-tooltip :content="bookStore.currentBook?.title || '未命名书籍'" placement="bottom">
        <h2 class="book-title">{{ bookStore.currentBook?.title || '未命名书籍' }}</h2>
      </el-tooltip>
    </header>

    <!-- 模式切换标签 -->
    <nav class="mode-tabs">
      <button 
        class="mode-tab" 
        :class="{ active: sidebarMode === 'chapters' }"
        @click="switchMode('chapters')"
      >
        <el-icon><Document /></el-icon>
        <span>章节</span>
      </button>
      <button 
        class="mode-tab" 
        :class="{ active: sidebarMode === 'outline' }"
        @click="switchMode('outline')"
      >
        <el-icon><Reading /></el-icon>
        <span>大纲</span>
      </button>
    </nav>

    <!-- 工具栏 -->
    <div class="toolbar">
      <el-button
        type="primary"
        plain
        :icon="FolderAdd"
        size="small"
        @click="openNewVolumeDialog"
        class="toolbar-btn"
      >
        新建卷
      </el-button>
      <el-button
        v-if="sidebarMode === 'chapters'"
        type="info"
        plain
        :icon="Collection"
        size="small"
        @click="$emit('show-snapshots')"
        class="toolbar-btn"
      >
        快照
      </el-button>
    </div>

    <!-- 内容区 -->
    <div class="content-area">
      <!-- 统一的树形结构 -->
      <el-tree
        :data="treeData"
        :props="{ children: 'children', label: 'label' }"
        :default-expanded-keys="defaultExpandedKeys"
        :highlight-current="sidebarMode === 'chapters'"
        :current-node-key="sidebarMode === 'chapters' ? currentChapterId : undefined"
        node-key="id"
        @node-click="handleNodeClick"
        @node-expand="handleNodeExpand"
        @node-collapse="handleNodeCollapse"
        class="chapter-tree"
      >
        <template #default="{ data }">
          <span class="tree-node">
            <span class="node-main">
              <el-icon v-if="data.type === 'volume'" class="node-icon folder-icon">
                <Folder />
              </el-icon>
              <el-icon v-else class="node-icon document-icon">
                <Document />
              </el-icon>
              
              <!-- 行内编辑（仅在章节模式） -->
              <template v-if="sidebarMode === 'chapters' && editingNodeId === data.id">
                <span class="inline-edit" @click.stop>
                  <span class="edit-prefix">第</span>
                  <input
                    ref="numberInputRef"
                    v-model="editingNumber"
                    class="edit-number"
                    @keyup.enter="confirmEdit"
                    @keyup.esc="cancelEdit"
                  />
                  <span class="edit-suffix">{{ data.type === 'volume' ? '卷' : '章' }}</span>
                  <input
                    ref="nameInputRef"
                    v-model="editingName"
                    class="edit-name"
                    placeholder="请输入标题"
                    @keyup.enter="confirmEdit"
                    @keyup.esc="cancelEdit"
                  />
                </span>
              </template>
              
              <span v-else class="node-label" :title="data.label">{{ data.label }}</span>
            </span>
            
            <!-- 操作按钮（仅在章节模式） -->
            <span v-if="sidebarMode === 'chapters' && editingNodeId !== data.id" class="node-actions">
              <template v-if="data.type === 'volume'">
                <el-button
                  link
                  size="small"
                  :icon="Plus"
                  title="添加章节"
                  @click.stop="handleCreateChapter(data.id)"
                />
                <el-button
                  link
                  size="small"
                  :icon="Edit"
                  title="重命名"
                  @click.stop="startEditing(data)"
                />
                <el-button
                  link
                  type="danger"
                  size="small"
                  :icon="Delete"
                  title="删除"
                  @click.stop="handleDeleteVolume(data.id)"
                />
              </template>
              <template v-else>
                <el-button
                  link
                  type="danger"
                  size="small"
                  :icon="Delete"
                  title="删除"
                  @click.stop="handleDeleteChapter(data.id)"
                />
              </template>
            </span>
            
            <!-- 大纲模式箭头 -->
            <span v-if="sidebarMode === 'outline'" class="node-arrow">
              <el-icon><ArrowRight /></el-icon>
            </span>
          </span>
        </template>
      </el-tree>

      <!-- 空状态 -->
      <el-empty v-if="treeData.length === 0" description="暂无卷">
        <template #image>
          <el-icon :size="48" color="var(--nw-text-tertiary)"><FolderAdd /></el-icon>
        </template>
        <p class="empty-hint">点击上方"新建卷"按钮创建</p>
      </el-empty>
    </div>

    <!-- 底部统计 -->
    <footer class="sidebar-footer">
      <div class="stat-row">
        <span class="stat-label">总字数</span>
        <span class="stat-value">{{ bookStore.totalWordCount.toLocaleString() }}</span>
      </div>
    </footer>

    <!-- 新建卷对话框 -->
    <el-dialog
      v-model="newVolumeDialogVisible"
      title="新建卷"
      width="400px"
      :close-on-click-modal="false"
      class="volume-dialog"
    >
      <div class="volume-form">
        <div class="title-composer">
          <span class="composer-prefix">第</span>
          <el-input
            ref="newVolumeNumberRef"
            v-model="newVolumeNumber"
            class="composer-number"
            @keyup.enter="confirmNewVolume"
          />
          <span class="composer-suffix">卷</span>
          <el-input
            v-model="newVolumeName"
            class="composer-name"
            placeholder="请输入卷名"
            @keyup.enter="confirmNewVolume"
          />
        </div>
      </div>
      <template #footer>
        <el-button @click="cancelNewVolume">取消</el-button>
        <el-button type="primary" @click="confirmNewVolume">创建</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.sidebar {
  width: 100%;
  height: 100%;
  background: var(--nw-bg-primary);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* 头部 */
.sidebar-header {
  display: flex;
  align-items: center;
  gap: var(--nw-space-sm);
  padding: var(--nw-space-md);
  border-bottom: 1px solid var(--nw-border-light);
  flex-shrink: 0;
}

.back-btn {
  flex-shrink: 0;
}

.book-title {
  font-size: 15px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  margin: 0;
  color: var(--nw-text-primary);
  font-family: var(--nw-font-display);
}

/* 模式切换 */
.mode-tabs {
  display: flex;
  border-bottom: 1px solid var(--nw-border-light);
  flex-shrink: 0;
}

.mode-tab {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: var(--nw-space) var(--nw-space-md);
  cursor: pointer;
  color: var(--nw-text-primary);
  font-size: 13px;
  font-weight: 500;
  transition: all var(--nw-transition-fast);
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  background: none;
  border: none;
  font-family: inherit;
}

.mode-tab:hover {
  color: var(--nw-primary);
  background: var(--nw-bg-hover);
}

.mode-tab.active {
  color: var(--nw-primary);
  border-bottom-color: var(--nw-primary);
  background: var(--nw-bg-secondary);
}

/* 工具栏 */
.toolbar {
  display: flex;
  gap: var(--nw-space-sm);
  padding: var(--nw-space);
  border-bottom: 1px solid var(--nw-border-light);
  flex-shrink: 0;
}

.toolbar-btn {
  flex: 1;
}

.outline-toolbar {
  justify-content: center;
}

.toolbar-title {
  font-size: 13px;
  color: var(--nw-text-tertiary);
  font-weight: 500;
}

/* 内容区 */
.content-area {
  flex: 1;
  overflow-y: auto;
  padding: var(--nw-space-sm) 0;
}

/* 章节树 */
.chapter-tree {
  background: transparent;
}

.chapter-tree :deep(.el-tree-node__content) {
  height: 40px;
  color: var(--nw-text-secondary);
  border-radius: var(--nw-radius-sm);
  margin: 0 var(--nw-space-sm);
  padding-right: var(--nw-space-sm);
}

.chapter-tree :deep(.el-tree-node__content:hover) {
  background: var(--nw-bg-hover);
}

.chapter-tree :deep(.el-tree-node:focus > .el-tree-node__content) {
  background: var(--nw-bg-tertiary);
}

.chapter-tree :deep(.el-tree-node.is-current > .el-tree-node__content) {
  background: var(--nw-bg-tertiary);
  color: var(--nw-primary-lighter);
}

.chapter-tree :deep(.el-tree-node__expand-icon) {
  color: var(--nw-text-muted);
}

.chapter-tree :deep(.el-tree-node__expand-icon.is-leaf) {
  color: transparent;
}

.tree-node {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex: 1;
  overflow: hidden;
}

.node-main {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  overflow: hidden;
}

.node-icon {
  font-size: 16px;
  flex-shrink: 0;
  color: var(--nw-text-muted);
}

.chapter-tree :deep(.el-tree-node.is-current) .document-icon {
  color: var(--nw-primary-lighter);
}

.node-label {
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 行内编辑 */
.inline-edit {
  display: flex;
  align-items: center;
  gap: 4px;
  flex: 1;
  min-width: 0;
}

.edit-prefix,
.edit-suffix {
  font-size: 13px;
  color: var(--nw-text-tertiary);
  flex-shrink: 0;
}

.edit-number {
  width: 36px;
  background: var(--nw-bg-secondary);
  border: 1px solid var(--nw-border);
  border-radius: var(--nw-radius-sm);
  padding: 2px 4px;
  font-size: 13px;
  color: var(--nw-text-primary);
  text-align: center;
  outline: none;
  font-family: inherit;
}

.edit-number:focus {
  border-color: var(--nw-primary);
}

.edit-name {
  flex: 1;
  background: var(--nw-bg-secondary);
  border: 1px solid var(--nw-border);
  border-radius: var(--nw-radius-sm);
  padding: 2px 8px;
  font-size: 13px;
  color: var(--nw-text-primary);
  outline: none;
  min-width: 0;
  font-family: inherit;
}

.edit-name:focus {
  border-color: var(--nw-primary);
}

.edit-name::placeholder {
  color: var(--nw-text-muted);
}

/* 节点操作按钮 */
.node-actions {
  display: flex;
  gap: 2px;
  opacity: 0;
  transition: opacity var(--nw-transition-fast);
}

.chapter-tree :deep(.el-tree-node__content:hover) .node-actions {
  opacity: 1;
}

.node-actions :deep(.el-button) {
  color: var(--nw-text-muted) !important;
  padding: 2px !important;
}

.node-actions :deep(.el-button:hover) {
  color: var(--nw-text-primary) !important;
}

.node-actions :deep(.el-button.el-button--danger) {
  color: var(--nw-danger) !important;
}

.node-actions :deep(.el-button.el-button--danger:hover) {
  color: var(--nw-danger) !important;
}

/* 大纲模式箭头 */
.node-arrow {
  display: flex;
  align-items: center;
  color: var(--nw-text-muted);
  opacity: 0;
  transition: opacity var(--nw-transition-fast);
}

.chapter-tree :deep(.el-tree-node__content:hover) .node-arrow {
  opacity: 1;
}

/* 空状态 */
.empty-hint {
  font-size: 12px;
  color: var(--nw-text-tertiary);
  margin-top: var(--nw-space-sm);
}

/* 底部 */
.sidebar-footer {
  padding: var(--nw-space) var(--nw-space-md);
  border-top: 1px solid var(--nw-border-light);
  background: var(--nw-bg-secondary);
  flex-shrink: 0;
}

.stat-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.stat-label {
  font-size: 12px;
  color: var(--nw-text-tertiary);
}

.stat-value {
  font-size: 14px;
  font-weight: 600;
  color: var(--nw-primary);
  font-family: var(--nw-font-mono);
}

/* 对话框 */
.volume-dialog :deep(.el-dialog__body) {
  padding: var(--nw-space-lg);
}

.volume-form {
  padding: var(--nw-space) 0;
}

.title-composer {
  display: flex;
  align-items: center;
  gap: var(--nw-space-sm);
}

.composer-prefix,
.composer-suffix {
  font-size: 16px;
  color: var(--nw-text-secondary);
  flex-shrink: 0;
}

.composer-number {
  width: 60px;
}

.composer-number :deep(.el-input__wrapper) {
  box-shadow: none;
  border-bottom: 2px solid var(--nw-border);
  border-radius: 0;
  padding: 0 4px;
}

.composer-number :deep(.el-input__inner) {
  text-align: center;
  font-size: 16px;
  font-weight: 500;
}

.composer-name {
  flex: 1;
}

.composer-name :deep(.el-input__wrapper) {
  box-shadow: none;
  border-bottom: 2px solid var(--nw-border);
  border-radius: 0;
  padding: 0 4px;
}

.composer-name :deep(.el-input__inner) {
  font-size: 16px;
}

.composer-number :deep(.el-input__wrapper:hover),
.composer-number :deep(.el-input__wrapper.is-focus),
.composer-name :deep(.el-input__wrapper:hover),
.composer-name :deep(.el-input__wrapper.is-focus) {
  border-bottom-color: var(--nw-primary);
}

/* 滚动条 */
.content-area::-webkit-scrollbar {
  width: 4px;
}

.content-area::-webkit-scrollbar-track {
  background: transparent;
}

.content-area::-webkit-scrollbar-thumb {
  background: var(--nw-border);
  border-radius: 2px;
}

.content-area::-webkit-scrollbar-thumb:hover {
  background: var(--nw-text-muted);
}

/* 深色模式强制覆盖 */
html.dark .mode-tab {
  color: #e6edf3 !important;
}

html.dark .mode-tab:hover {
  color: #7aafd5 !important;
}

html.dark .mode-tab.active {
  color: #7aafd5 !important;
}

html.dark .toolbar-btn {
  color: #e6edf3 !important;
}

/* 深色模式下选中章节 */
html.dark .chapter-tree .el-tree-node.is-current > .el-tree-node__content {
  color: #7aafd5 !important;
}

html.dark .chapter-tree .el-tree-node.is-current .document-icon {
  color: #7aafd5 !important;
}

/* 深色模式全局文字颜色修复 */
html.dark .chapter-tree :deep(.el-tree-node__content) {
  color: #b4bcc5 !important;
}

html.dark .node-icon,
html.dark .chapter-tree :deep(.el-tree-node__expand-icon) {
  color: #8b949e !important;
}

html.dark .empty-hint,
html.dark .stat-label,
html.dark .toolbar-title,
html.dark .outline-hint {
  color: #8b949e !important;
}

html.dark .node-actions :deep(.el-button) {
  color: #8b949e !important;
}

html.dark .node-actions :deep(.el-button:hover) {
  color: #e6edf3 !important;
}

html.dark .item-icon,
html.dark .item-arrow {
  color: #6e7681 !important;
}

html.dark .outline-item:hover .item-arrow {
  color: #b4bcc5 !important;
}
</style>
