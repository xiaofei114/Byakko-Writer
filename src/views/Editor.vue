<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import Sidebar from '../components/Sidebar.vue';
import WritingArea from '../components/WritingArea.vue';
import AIChat from '../components/AIChat.vue';
import SnapshotManager from '../components/SnapshotManager.vue';
import CharacterCardManager from '../components/CharacterCardManager.vue';
import QuickReferencePanel from '../components/QuickReferencePanel.vue';
import ConflictDialog from '../components/ConflictDialog.vue';
import ConflictSidebar from '../components/ConflictSidebar.vue';
import { useBookStore } from '../stores/book';
import { ChatRound, User, Document, ArrowLeft, Check, View, Edit, WarningFilled } from '@element-plus/icons-vue';
import { ElMessage, ElNotification } from 'element-plus';
import { marked } from 'marked';

marked.setOptions({
  breaks: true,
  gfm: true
});

const bookStore = useBookStore();

const showAIChat = ref(true);
const aiChatWidth = ref(320);
const isResizing = ref(false);
const sidebarWidth = ref(260);
const sidebarResizing = ref(false);
const showSnapshotManager = ref(false);

// 角色卡和大纲相关
const showCharacterCardManager = ref(false);
const showQuickOutlinePanel = ref(false);

// 大纲编辑模式
const showOutlineEditing = ref(false);
const outlineEditTitle = ref('');
const coarseOutlineContent = ref('');
const fineOutlineContent = ref('');
const currentOutlineTarget = ref<{
  level: 'book' | 'volume' | 'chapter';
  targetId?: string;
} | null>(null);
let autoSaveTimer: ReturnType<typeof setTimeout> | null = null;
const outlineDirty = ref(false);

// 大纲 Markdown 预览模式
const coarsePreviewMode = ref(true);
const finePreviewMode = ref(true);

const renderMarkdown = (content: string) => {
  if (!content) return '';
  return marked.parse(content);
};

// 自动保存大纲
const autoSaveOutline = () => {
  outlineDirty.value = true;
  if (autoSaveTimer) clearTimeout(autoSaveTimer);
  autoSaveTimer = setTimeout(() => {
    saveOutlineInternal();
  }, 2000);
};

// 内部保存函数
const saveOutlineInternal = async () => {
  if (!bookStore.currentBook || !currentOutlineTarget.value || !outlineDirty.value) return;
  
  outlineDirty.value = false;
  const { level, targetId } = currentOutlineTarget.value;
  
  try {
    await Promise.all([
      bookStore.saveOutline({
        book_id: bookStore.currentBook.id,
        volume_id: level === 'volume' ? targetId : undefined,
        chapter_id: level === 'chapter' ? targetId : undefined,
        outline_type: 'coarse',
        content: coarseOutlineContent.value
      }),
      bookStore.saveOutline({
        book_id: bookStore.currentBook.id,
        volume_id: level === 'volume' ? targetId : undefined,
        chapter_id: level === 'chapter' ? targetId : undefined,
        outline_type: 'fine',
        content: fineOutlineContent.value
      })
    ]);
  } catch (error) {
    console.error('自动保存大纲失败:', error);
  }
};

// Ctrl+S 保存
const handleKeydown = (e: KeyboardEvent) => {
  if ((e.ctrlKey || e.metaKey) && e.key === 's') {
    e.preventDefault();
    if (showOutlineEditing.value) {
      saveOutlineInternal();
      ElMessage.success('大纲已保存');
    }
  }
};

// 手动保存按钮
const handleManualSave = async () => {
  outlineDirty.value = true;
  if (autoSaveTimer) clearTimeout(autoSaveTimer);
  await saveOutlineInternal();
  ElMessage.success('大纲已保存');
};

// 处理选择大纲
const handleSelectOutline = async (
  level: 'book' | 'volume' | 'chapter',
  targetId?: string
) => {
  if (!bookStore.currentBook) return;
  
  // 保存当前大纲（如果有）
  if (autoSaveTimer) {
    clearTimeout(autoSaveTimer);
    await saveOutlineInternal();
  }
  
  currentOutlineTarget.value = { level, targetId };
  
  let levelText = '';
  let targetName = '';
  
  if (level === 'book') {
    levelText = '书籍';
    targetName = bookStore.currentBook.title;
  } else if (level === 'volume' && targetId) {
    levelText = '卷';
    const volume = bookStore.volumes.find(v => v.id === targetId);
    targetName = volume?.title || '';
  } else if (level === 'chapter' && targetId) {
    levelText = '章节';
    const chapter = bookStore.chapters.find(c => c.id === targetId);
    targetName = chapter?.title || '';
  }
  
  outlineEditTitle.value = `${targetName} - ${levelText}大纲`;
  
  try {
    const [coarse, fine] = await Promise.all([
      bookStore.getOutlineByLevel(
        level === 'volume' ? targetId : undefined,
        level === 'chapter' ? targetId : undefined,
        'coarse'
      ),
      bookStore.getOutlineByLevel(
        level === 'volume' ? targetId : undefined,
        level === 'chapter' ? targetId : undefined,
        'fine'
      ),
    ]);
    coarseOutlineContent.value = coarse?.content || '';
    fineOutlineContent.value = fine?.content || '';
  } catch (error) {
    console.error('加载大纲失败:', error);
    coarseOutlineContent.value = '';
    fineOutlineContent.value = '';
  }
  
  outlineDirty.value = false;
  showOutlineEditing.value = true;
};

// 关闭大纲编辑
const closeOutlineEditing = async () => {
  showOutlineEditing.value = false;
  if (autoSaveTimer) {
    clearTimeout(autoSaveTimer);
    await saveOutlineInternal();
  }
  coarseOutlineContent.value = '';
  fineOutlineContent.value = '';
  currentOutlineTarget.value = null;
  outlineDirty.value = false;
};

// 处理章节树点击：如果在编辑大纲中，保存并退出
const handleSelectChapter = async () => {
  if (!showOutlineEditing.value) return;
  showOutlineEditing.value = false;
  if (autoSaveTimer) {
    clearTimeout(autoSaveTimer);
    await saveOutlineInternal();
  }
  coarseOutlineContent.value = '';
  fineOutlineContent.value = '';
  currentOutlineTarget.value = null;
  outlineDirty.value = false;
};

// 监听章节切换：如果在大纲编辑中点击了章节树，关闭大纲回到写作
watch(() => bookStore.currentChapterId, async () => {
  if (showOutlineEditing.value) {
    showOutlineEditing.value = false;
    if (autoSaveTimer) {
      clearTimeout(autoSaveTimer);
      await saveOutlineInternal();
    }
    coarseOutlineContent.value = '';
    fineOutlineContent.value = '';
    currentOutlineTarget.value = null;
    outlineDirty.value = false;
  }
});

const goHome = async () => {
  // 关闭书籍前为当前章节创建快照
  if (bookStore.currentChapterId) {
    await bookStore.createImmediateSnapshot('关闭书籍', bookStore.currentChapterId);
  }
  bookStore.currentBook = null;
  bookStore.currentChapterId = null;
};

// 开始拖拽调整右边栏
const startResizeRight = (e: MouseEvent) => {
  e.preventDefault();
  isResizing.value = true;
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
};

// 开始拖拽调整左边栏
const startResizeLeft = (e: MouseEvent) => {
  e.preventDefault();
  sidebarResizing.value = true;
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
};

// 处理拖拽
const handleMouseMove = (e: MouseEvent) => {
  if (isResizing.value) {
    const newWidth = window.innerWidth - e.clientX;
    aiChatWidth.value = Math.max(250, Math.min(600, newWidth));
  }
  if (sidebarResizing.value) {
    sidebarWidth.value = Math.max(180, Math.min(400, e.clientX));
  }
};

// 停止拖拽
const stopResize = () => {
  isResizing.value = false;
  sidebarResizing.value = false;
  document.body.style.cursor = '';
  document.body.style.userSelect = '';
};

// 监听大纲保存事件（从 AIChat 组件发送）
const handleOutlineSaved = (event: CustomEvent<{ chapterId: string; outlineType: string }>) => {
  const { chapterId } = event.detail;

  // 如果当前正在编辑该章节的大纲，刷新数据
  if (showOutlineEditing.value &&
      currentOutlineTarget.value?.level === 'chapter' &&
      currentOutlineTarget.value?.targetId === chapterId) {
    // 重新加载大纲数据
    handleSelectOutline('chapter', chapterId);
  }
};

// 冲突检测
import type { DetectedConflict } from '../types';
const showConflictDialog = ref(false);
const showConflictSidebar = ref(false);
const conflictList = ref<DetectedConflict[]>([]);

const checkConflictsInBackground = async () => {
  if (!bookStore.currentBook) return;
  try {
    const conflicts = await invoke<DetectedConflict[]>('get_active_conflicts', { bookId: bookStore.currentBook.id });
    if (conflicts.length > 0) {
      conflictList.value = conflicts;
      showConflictDialog.value = true;
      ElNotification({
        title: '设定冲突检测',
        message: `检测到 ${conflicts.length} 个剧情冲突，请及时查看`,
        type: 'warning',
        duration: 5000,
      });
    }
  } catch (e) {
    // 后台静默，忽略错误
  }
};

const handleConflictIgnored = (conflictId: string) => {
  conflictList.value = conflictList.value.filter(c => c.id !== conflictId);
  if (conflictList.value.length === 0) {
    showConflictDialog.value = false;
  }
};

onMounted(() => {
  document.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', stopResize);
  document.addEventListener('keydown', handleKeydown);

  // 监听页面关闭/刷新事件
  window.addEventListener('beforeunload', handleBeforeUnload);

  // 监听大纲保存事件
  window.addEventListener('outline-saved', handleOutlineSaved as EventListener);

  // 后台静默执行冲突检测
  checkConflictsInBackground();
});

onUnmounted(() => {
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResize);
  document.removeEventListener('keydown', handleKeydown);
  window.removeEventListener('beforeunload', handleBeforeUnload);
  window.removeEventListener('outline-saved', handleOutlineSaved as EventListener);
});

// 页面关闭前创建快照
const handleBeforeUnload = (_e: BeforeUnloadEvent) => {
  if (bookStore.hasUnsavedChanges && bookStore.currentChapterId) {
    // 同步创建快照（beforeunload 中只能同步操作）
    bookStore.createImmediateSnapshot('关闭应用', bookStore.currentChapterId);
  }
};
</script>

<template>
  <div class="editor-layout">
    <!-- 左侧边栏 -->
    <aside class="sidebar-panel" :style="{ width: sidebarWidth + 'px' }">
      <Sidebar 
        :show-outline-editing="showOutlineEditing"
        @go-home="goHome" 
        @show-snapshots="showSnapshotManager = true"
        @select-outline="handleSelectOutline"
        @select-chapter="handleSelectChapter"
      />
    </aside>
    
    <!-- 左侧拖拽条 -->
    <div class="resize-handle left" @mousedown="startResizeLeft">
      <div class="resize-indicator"></div>
    </div>
    
    <!-- 主内容区 -->
    <main class="editor-main">
      <!-- 正常写作模式 -->
      <WritingArea v-if="!showOutlineEditing" />
      
      <!-- 大纲编辑模式 -->
      <div v-else class="outline-editor">
        <header class="outline-header">
          <div class="header-left">
            <el-button text :icon="ArrowLeft" @click="closeOutlineEditing">
              返回写作
            </el-button>
            <div class="header-divider"></div>
            <span class="outline-title">{{ outlineEditTitle }}</span>
          </div>
          <el-button type="primary" :icon="Check" @click="handleManualSave">
            保存大纲
          </el-button>
        </header>
        
        <div class="outline-content">
          <!-- 粗纲 -->
          <section class="outline-section">
            <div class="section-toolbar">
              <span class="section-label">粗纲</span>
              <el-button
                text
                size="small"
                :icon="coarsePreviewMode ? Edit : View"
                @click="coarsePreviewMode = !coarsePreviewMode"
              >
                {{ coarsePreviewMode ? '编辑' : '预览' }}
              </el-button>
            </div>
            <div class="section-body">
              <el-input
                v-if="!coarsePreviewMode"
                v-model="coarseOutlineContent"
                type="textarea"
                class="outline-input"
                placeholder="编写粗纲，梳理故事主线..."
                @input="autoSaveOutline"
              />
              <div
                v-else
                class="outline-preview markdown-body"
                v-html="renderMarkdown(coarseOutlineContent)"
              ></div>
            </div>
          </section>
          
          <!-- 分隔条 -->
          <div class="outline-splitter"></div>
          
          <!-- 细纲 -->
          <section class="outline-section">
            <div class="section-toolbar">
              <span class="section-label">细纲</span>
              <el-button
                text
                size="small"
                :icon="finePreviewMode ? Edit : View"
                @click="finePreviewMode = !finePreviewMode"
              >
                {{ finePreviewMode ? '编辑' : '预览' }}
              </el-button>
            </div>
            <div class="section-body">
              <el-input
                v-if="!finePreviewMode"
                v-model="fineOutlineContent"
                type="textarea"
                class="outline-input"
                placeholder="编写细纲，细化情节发展..."
                @input="autoSaveOutline"
              />
              <div
                v-else
                class="outline-preview markdown-body"
                v-html="renderMarkdown(fineOutlineContent)"
              ></div>
            </div>
          </section>
        </div>
      </div>
    </main>
    
    <!-- 右侧拖拽条 -->
    <div v-if="showAIChat" class="resize-handle right" @mousedown="startResizeRight">
      <div class="resize-indicator"></div>
    </div>
    
    <!-- AI 面板 -->
    <aside v-if="showAIChat" class="ai-panel" :style="{ width: aiChatWidth + 'px' }">
      <AIChat @close="showAIChat = false" />
    </aside>
    
    <!-- 显示 AI 按钮 -->
    <el-button
      v-else
      class="ai-toggle-btn"
      type="primary"
      round
      :icon="ChatRound"
      @click="showAIChat = true"
    >
      AI助手
    </el-button>

    <!-- 快捷工具 -->
    <div class="floating-tools">
      <el-tooltip content="角色卡管理" placement="left">
        <el-button
          circle
          type="primary"
          :icon="User"
          @click="showCharacterCardManager = true"
        />
      </el-tooltip>
      <el-tooltip content="本章大纲" placement="left">
        <el-button
          circle
          type="success"
          :icon="Document"
          @click="showQuickOutlinePanel = true"
        />
      </el-tooltip>
      <el-tooltip content="检测设定冲突" placement="left">
        <el-button
          circle
          type="warning"
          :icon="WarningFilled"
          @click="showConflictSidebar = true"
        />
      </el-tooltip>
    </div>

    <!-- 弹窗组件 -->
    <SnapshotManager v-model="showSnapshotManager" />
    <CharacterCardManager v-model="showCharacterCardManager" />
    <QuickReferencePanel 
      v-model="showQuickOutlinePanel" 
      type="outline"
      @open-full-manager="showQuickOutlinePanel = false"
    />

    <!-- 设定冲突检测弹窗 -->
    <ConflictDialog
      :visible="showConflictDialog"
      :conflicts="conflictList"
      :book-id="bookStore.currentBook?.id || ''"
      @close="showConflictDialog = false"
      @ignored="handleConflictIgnored"
    />
    <!-- 设定冲突检测 -->
    <ConflictSidebar v-model="showConflictSidebar" />
  </div>
</template>

<style scoped>
.editor-layout {
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  display: flex;
  background: var(--nw-bg-page);
}

/* 侧边栏面板 */
.sidebar-panel {
  flex-shrink: 0;
  background: var(--nw-bg-primary);
  border-right: 1px solid var(--nw-border-light);
  overflow: hidden;
}

/* 主编辑区 */
.editor-main {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* AI 面板 */
.ai-panel {
  flex-shrink: 0;
  background: var(--nw-bg-primary);
  border-left: 1px solid var(--nw-border-light);
  overflow: hidden;
}

/* 拖拽条 */
.resize-handle {
  width: 8px;
  background: transparent;
  cursor: col-resize;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background var(--nw-transition-fast);
  z-index: 10;
}

.resize-handle:hover {
  background: var(--nw-bg-hover);
}

.resize-indicator {
  width: 2px;
  height: 32px;
  background: var(--nw-border);
  border-radius: 1px;
  transition: background var(--nw-transition-fast);
}

.resize-handle:hover .resize-indicator {
  background: var(--nw-primary-light);
}

/* AI 切换按钮 */
.ai-toggle-btn {
  position: fixed;
  right: var(--nw-space-lg);
  bottom: var(--nw-space-lg);
  z-index: 100;
  box-shadow: var(--nw-shadow-md);
}

/* 浮动工具 */
.floating-tools {
  position: fixed;
  right: var(--nw-space-lg);
  top: 50%;
  transform: translateY(-50%);
  z-index: 100;
  display: flex;
  flex-direction: column;
  gap: var(--nw-space-sm);
}

.floating-tools .el-button {
  box-shadow: var(--nw-shadow-md);
  margin: 0;
}

/* 大纲编辑器 */
.outline-editor {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--nw-bg-page);
}

.outline-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--nw-space-md) var(--nw-space-lg);
  background: var(--nw-bg-primary);
  border-bottom: 1px solid var(--nw-border-light);
  flex-shrink: 0;
}

.header-left {
  display: flex;
  align-items: center;
  gap: var(--nw-space-md);
}

.header-divider {
  width: 1px;
  height: 20px;
  background: var(--nw-border);
}

.outline-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--nw-text-primary);
  font-family: var(--nw-font-display);
}

.outline-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: var(--nw-space-lg);
  gap: var(--nw-space-lg);
}

.outline-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: var(--nw-bg-primary);
  border-radius: var(--nw-radius-md);
  border: 1px solid var(--nw-border-light);
  overflow: hidden;
  min-height: 0;
}

.section-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--nw-space) var(--nw-space-md);
  background: var(--nw-bg-secondary);
  border-bottom: 1px solid var(--nw-border-light);
  flex-shrink: 0;
}

.section-label {
  font-size: 14px;
  font-weight: 600;
  color: var(--nw-text-primary);
  font-family: var(--nw-font-display);
}

.section-body {
  flex: 1;
  overflow: hidden;
  min-height: 0;
}

.outline-input {
  height: 100%;
}

.outline-input :deep(.el-textarea__inner) {
  height: 100% !important;
  resize: none;
  font-size: 15px;
  line-height: 1.8;
  padding: var(--nw-space-md);
  border: none;
  background: var(--nw-bg-primary);
  font-family: var(--nw-font-body);
}

.outline-preview {
  height: 100%;
  overflow-y: auto;
  padding: var(--nw-space-md);
  font-size: 15px;
  line-height: 1.8;
  color: var(--nw-text-primary);
  background: var(--nw-bg-primary);
}

.outline-preview :deep(h1),
.outline-preview :deep(h2),
.outline-preview :deep(h3),
.outline-preview :deep(h4) {
  margin-top: 1em;
  margin-bottom: 0.5em;
  font-weight: 600;
  color: var(--nw-text-primary);
  font-family: var(--nw-font-display);
}

.outline-preview :deep(p) {
  margin-bottom: 0.8em;
}

.outline-preview :deep(ul),
.outline-preview :deep(ol) {
  padding-left: 1.5em;
  margin-bottom: 0.8em;
}

.outline-preview :deep(li) {
  margin-bottom: 0.3em;
}

.outline-preview :deep(strong) {
  font-weight: 600;
  color: var(--nw-primary);
}

.outline-preview :deep(hr) {
  margin: 1em 0;
  border: none;
  border-top: 1px solid var(--nw-border-light);
}

.outline-preview :deep(code) {
  background: var(--nw-bg-secondary);
  padding: 2px 6px;
  border-radius: var(--nw-radius-sm);
  font-size: 0.9em;
  font-family: var(--nw-font-mono);
}

.outline-preview :deep(blockquote) {
  border-left: 3px solid var(--nw-accent);
  padding-left: var(--nw-space-md);
  margin-left: 0;
  color: var(--nw-text-secondary);
  font-style: italic;
}

.outline-splitter {
  height: 1px;
  background: var(--nw-border-light);
  flex-shrink: 0;
}
</style>
