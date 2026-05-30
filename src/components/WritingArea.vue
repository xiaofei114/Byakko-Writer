<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { useBookStore } from '../stores/book';
import { useConfigStore } from '../stores/config';
import { generateChapterSummary, loadChapterSummary } from '../api/aiFeatures';
import type { ChapterSummary } from '../types';
import { Check, Warning, Loading, Collection, Folder, Document, View, MagicStick } from '@element-plus/icons-vue';
import { ElMessage } from 'element-plus';
import { requestPolish } from '../utils/eventBus';

const bookStore = useBookStore();
const configStore = useConfigStore();

const editorRef = ref<HTMLDivElement>();
const saveStatus = ref<'saved' | 'saving' | 'unsaved'>('saved');
const isLoading = ref(false);



// 摘要相关
const chapterSummary = ref<ChapterSummary | null>(null);
const isGeneratingSummary = ref(false);
const showSummaryPanel = ref(false);

// 润色相关
const showPolishToolbar = ref(false);
const polishToolbarPos = ref({ x: 0, y: 0 });
const selectedText = ref('');
const isPolishing = ref(false);

// 缓存当前章节ID，避免重复加载
const lastLoadedChapterId = ref<string | null>(null);

// 将纯文本转换为带 p 标签的 HTML
const textToHtml = (text: string): string => {
  if (!text) return '<p><br></p>';
  const lines = text.split('\n');
  let html = '';
  for (let i = 0; i < lines.length; i++) {
    const content = lines[i].trim() || '<br>';
    html += `<p>${content}</p>`;
  }
  return html;
};

// 将 HTML 转换回纯文本
let tempDiv: HTMLDivElement | null = null;
const getTempDiv = (): HTMLDivElement => {
  if (!tempDiv) {
    tempDiv = document.createElement('div');
  }
  return tempDiv;
};

const htmlToText = (html: string): string => {
  const div = getTempDiv();
  div.innerHTML = html;
  const paragraphs = div.querySelectorAll('p');
  let text = '';
  for (let i = 0; i < paragraphs.length; i++) {
    if (i > 0) text += '\n';
    text += paragraphs[i].textContent || '';
  }
  return text;
};

// 加载章节内容
const loadContent = async () => {
  const chapterId = bookStore.currentChapterId;
  if (!chapterId || !editorRef.value) return;
  
  if (lastLoadedChapterId.value === chapterId) return;
  
  isLoading.value = true;
  lastLoadedChapterId.value = chapterId;
  
  await nextTick();
  
  updateChapterTitleDisplay(bookStore.currentChapter?.title);
  
  if (editorRef.value) {
    editorRef.value.innerHTML = textToHtml(bookStore.currentContent);
    saveStatus.value = 'saved';
  }
  
  isLoading.value = false;
};

// 监听章节变化
watch(() => bookStore.currentChapterId, async (newId, oldId) => {
  if (newId !== oldId) {
    if (oldId && bookStore.hasUnsavedChanges) {
      await bookStore.createImmediateSnapshot('切换章节', oldId);
    }
    bookStore.clearModified(newId || undefined);
    loadContent();
  }
});

// 监听内容变化
let isUpdatingFromEditor = false;
watch(() => bookStore.currentContent, (newContent) => {
  if (isUpdatingFromEditor) return;
  
  if (editorRef.value && newContent !== htmlToText(editorRef.value.innerHTML)) {
    editorRef.value.innerHTML = textToHtml(newContent);
    saveStatus.value = 'saved';
  }
});

const wordCount = computed(() => bookStore.wordCount);
const volumeTitle = computed(() => {
  const chapter = bookStore.currentChapter;
  if (!chapter) return '';
  const volume = bookStore.currentBook?.volumes.find(v => v.id === chapter.volumeId);
  return volume?.title || '';
});

// 章节标题编辑
const chapterNumber = ref('');
const chapterName = ref('');

const parseChapterTitle = (title: string) => {
  const match = title.match(/^第\s*(\d+)\s*章\s*(.*)$/);
  if (match) {
    return { number: match[1], name: match[2].trim() };
  }
  const numMatch = title.match(/^(\d+)\s*[\.、\s]\s*(.*)$/);
  if (numMatch) {
    return { number: numMatch[1], name: numMatch[2].trim() };
  }
  return { number: '', name: title };
};

const composeChapterTitle = () => {
  const num = chapterNumber.value.trim();
  const name = chapterName.value.trim();
  if (num && name) {
    return `第${num}章 ${name}`;
  } else if (num) {
    return `第${num}章`;
  } else if (name) {
    return name;
  }
  return '';
};

const updateChapterTitleDisplay = (title: string | undefined) => {
  if (title) {
    const parsed = parseChapterTitle(title);
    chapterNumber.value = parsed.number;
    chapterName.value = parsed.name;
  } else {
    chapterNumber.value = '';
    chapterName.value = '';
  }
};

watch(() => bookStore.currentChapter?.title, (newTitle) => {
  updateChapterTitleDisplay(newTitle);
}, { immediate: true });

const saveChapterTitle = async () => {
  if (bookStore.currentChapter) {
    const newTitle = composeChapterTitle();
    if (newTitle) {
      try {
        await bookStore.updateChapterTitle(bookStore.currentChapter.id, newTitle);
      } catch (error) {
        console.error('章节标题保存失败:', error);
      }
    }
  }
};

// 自动保存
let autoSaveTimer: number | null = null;

const stopAutoSave = () => {
  if (autoSaveTimer) {
    clearTimeout(autoSaveTimer);
    autoSaveTimer = null;
  }
};

const triggerAutoSave = () => {
  if (!configStore.autoSaveEnabled) return;
  
  stopAutoSave();
  autoSaveTimer = window.setTimeout(() => {
    if (saveStatus.value === 'unsaved') {
      performSave();
    }
  }, configStore.autoSaveIntervalMs);
};

const performSave = async () => {
  if (!bookStore.currentBook || !bookStore.currentChapter) return;

  saveStatus.value = 'saving';
  try {
    await bookStore.saveBook();
    saveStatus.value = 'saved';
  } catch (error) {
    console.error('保存失败:', error);
    saveStatus.value = 'unsaved';
    ElMessage.error('保存失败: ' + (error as Error).message);
  }
};

// 使用防抖处理输入
let inputDebounceTimer: number | null = null;
let lastHtmlContent = '';

const handleInput = () => {
  if (!editorRef.value) return;
  
  if (inputDebounceTimer) {
    clearTimeout(inputDebounceTimer);
  }
  
  inputDebounceTimer = window.setTimeout(() => {
    if (!editorRef.value) return;
    
    const html = editorRef.value.innerHTML;
    if (html === lastHtmlContent) return;
    lastHtmlContent = html;
    
    isUpdatingFromEditor = true;
    const text = htmlToText(html);
    bookStore.currentContent = text;
    saveStatus.value = 'unsaved';
    isUpdatingFromEditor = false;
    
    bookStore.markAsModified();
    triggerAutoSave();
  }, 100);
};

// 处理粘贴事件：只保留纯文本
const handlePaste = (e: ClipboardEvent) => {
  e.preventDefault();
  
  const text = e.clipboardData?.getData('text/plain') || '';
  if (!text || !editorRef.value) return;
  
  // 使用 execCommand 插入文本，自动推入浏览器撤销栈
  document.execCommand('insertText', false, text);
  
  handleInput();
};

// 处理键盘事件
const handleKeyDown = (e: KeyboardEvent) => {
  if ((e.ctrlKey || e.metaKey) && e.key === 's') {
    e.preventDefault();
    performSave();
    bookStore.createAutoSnapshot('Ctrl+S 保存');
    return;
  }
  
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    document.execCommand('defaultParagraphSeparator', false, 'p');
    document.execCommand('insertParagraph', false);
  }
};

// 监听配置变化
watch(() => configStore.autoSaveEnabled, (enabled) => {
  if (!enabled) {
    stopAutoSave();
  }
});

// 加载章节摘要
const loadSummary = async () => {
  if (!bookStore.currentChapterId) return;
  try {
    const summary = await loadChapterSummary(bookStore.currentChapterId);
    chapterSummary.value = summary;
  } catch (error) {
    console.error('加载摘要失败:', error);
  }
};

// 生成摘要
const handleGenerateSummary = async () => {
  if (!bookStore.currentChapter || !configStore.aiConfig.apiKey) {
    ElMessage.warning('请先配置 AI API 密钥');
    return;
  }

  const content = bookStore.currentContent;
  if (content.length < 50) {
    ElMessage.warning('章节内容太短（至少50字），无法生成摘要');
    return;
  }

  isGeneratingSummary.value = true;
  try {
    const summary = await generateChapterSummary(
      bookStore.currentChapter.id,
      bookStore.currentChapter.title,
      content,
      configStore.aiConfig
    );
    chapterSummary.value = summary;
    showSummaryPanel.value = true;
    ElMessage.success('摘要生成成功');
  } catch (error) {
    console.error('生成摘要失败:', error);
    ElMessage.error('生成摘要失败: ' + (error as Error).message);
  } finally {
    isGeneratingSummary.value = false;
  }
};

// 切换摘要面板
const toggleSummaryPanel = () => {
  showSummaryPanel.value = !showSummaryPanel.value;
  if (showSummaryPanel.value && !chapterSummary.value) {
    loadSummary();
  }
};

// 监听章节变化，加载摘要
watch(() => bookStore.currentChapterId, () => {
  chapterSummary.value = null;
  showSummaryPanel.value = false;
  showPolishToolbar.value = false;
  loadSummary();
});

// 处理选中文本
const handleSelectionChange = () => {
  const selection = window.getSelection();
  const text = selection?.toString().trim() || '';
  
  if (text && text.length > 0 && editorRef.value?.contains(selection?.anchorNode || null)) {
    selectedText.value = text;
    
    // 计算工具栏位置
    const range = selection?.getRangeAt(0);
    const rect = range?.getBoundingClientRect();
    if (rect) {
      polishToolbarPos.value = {
        x: rect.left + rect.width / 2,
        y: rect.top - 50
      };
      showPolishToolbar.value = true;
    }
  } else {
    showPolishToolbar.value = false;
  }
};

// 润色文本 - 发送到 AI 聊天
const polishText = () => {
  if (!selectedText.value) {
    return;
  }
  
  if (selectedText.value.length < 10) {
    ElMessage.warning('选中的文字太短（至少10字）');
    return;
  }
  
  // 发送润色请求到 AI 聊天
  requestPolish(selectedText.value, bookStore.currentChapterId || undefined);
  
  showPolishToolbar.value = false;
  // 清除选区
  window.getSelection()?.removeAllRanges();
  
  ElMessage.info('润色请求已发送到 AI 聊天');
};

// 处理应用润色事件
const handleApplyPolish = (event: CustomEvent<{ originalText: string; polishedText: string }>) => {
  const { polishedText } = event.detail;

  if (!editorRef.value) return;

  // 获取编辑器内容
  const content = htmlToText(editorRef.value.innerHTML);

  // 查找并替换原文（使用完整原文匹配）
  // 注意：这里使用简单的字符串替换，实际可能需要更精确的定位
  const fullOriginalText = selectedText.value;
  if (content.includes(fullOriginalText)) {
    const newContent = content.replace(fullOriginalText, polishedText);
    editorRef.value.innerHTML = textToHtml(newContent);

    // 更新 store
    isUpdatingFromEditor = true;
    bookStore.currentContent = newContent;
    isUpdatingFromEditor = false;

    // 触发保存
    saveStatus.value = 'unsaved';
    bookStore.markAsModified();
    triggerAutoSave();
  } else {
    ElMessage.error('无法定位原文，请手动复制粘贴');
  }
};

// 处理应用行编辑事件
const handleApplyLineEdit = (event: CustomEvent<{ chapterId: string; lineNumber: number; originalText: string; newText: string }>) => {
  const { chapterId, originalText, newText } = event.detail;

  // 确保是当前章节
  if (chapterId !== bookStore.currentChapterId) {
    ElMessage.warning('当前打开的章节与修改目标不一致');
    return;
  }

  if (!editorRef.value) return;

  // 获取编辑器内容
  const content = htmlToText(editorRef.value.innerHTML);
  const lines = content.split('\n');

  // 根据原文查找实际行号（不使用AI给的行号，因为可能不准确）
  const searchText = originalText.trim();
  let foundIndex = -1;

  // 首先尝试精确匹配
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].trim() === searchText) {
      foundIndex = i;
      break;
    }
  }

  // 如果精确匹配失败，尝试包含匹配
  if (foundIndex === -1) {
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].includes(searchText) || searchText.includes(lines[i].trim())) {
        // 选择相似度最高的行
        if (lines[i].trim().length > 0) {
          foundIndex = i;
          break;
        }
      }
    }
  }

  if (foundIndex === -1) {
    ElMessage.error('未找到原文所在行，请手动检查');
    console.log('查找原文:', searchText);
    return;
  }

  const actualLineNumber = foundIndex + 1; // 转换为1-based行号

  // 替换该行
  lines[foundIndex] = newText;
  const newContent = lines.join('\n');

  // 更新编辑器
  editorRef.value.innerHTML = textToHtml(newContent);

  // 更新 store
  isUpdatingFromEditor = true;
  bookStore.currentContent = newContent;
  isUpdatingFromEditor = false;

  // 触发保存
  saveStatus.value = 'unsaved';
  bookStore.markAsModified();
  triggerAutoSave();

  ElMessage.success(`第 ${actualLineNumber} 行已更新`);
};

// 处理章节内容变化事件（来自AI单行修改）
const handleChapterContentChanged = (event: CustomEvent) => {
  const { chapterId } = event.detail;
  // 如果变化的是当前章节，重新加载内容
  if (chapterId === bookStore.currentChapterId) {
    lastLoadedChapterId.value = ''; // 重置缓存，强制重新加载
    loadContent();
  }
};

onMounted(() => {
  loadContent();
  loadSummary();
  
  // 监听选中文本
  document.addEventListener('selectionchange', handleSelectionChange);
  
  // 监听应用润色事件
  window.addEventListener('apply-polish', handleApplyPolish as EventListener);

  // 监听应用行编辑事件
  window.addEventListener('apply-line-edit', handleApplyLineEdit as EventListener);

  // 监听章节内容变化事件（来自AI单行修改）
  window.addEventListener('chapter-content-changed', handleChapterContentChanged as EventListener);

  // 监听粘贴事件
  if (editorRef.value) {
    editorRef.value.addEventListener('paste', handlePaste);
  }
});

onUnmounted(() => {
  stopAutoSave();
  if (inputDebounceTimer) {
    clearTimeout(inputDebounceTimer);
  }
  tempDiv = null;
  
  // 移除监听
  document.removeEventListener('selectionchange', handleSelectionChange);
  window.removeEventListener('apply-polish', handleApplyPolish as EventListener);
  window.removeEventListener('apply-line-edit', handleApplyLineEdit as EventListener);
  window.removeEventListener('chapter-content-changed', handleChapterContentChanged as EventListener);

  if (editorRef.value) {
    editorRef.value.removeEventListener('paste', handlePaste);
  }
});
</script>

<template>
  <div class="writing-area">
    <!-- 顶部工具栏 -->
    <header class="editor-toolbar">
      <div class="toolbar-left">
        <div class="breadcrumb">
          <el-icon class="breadcrumb-icon"><Folder /></el-icon>
          <span class="breadcrumb-text">{{ volumeTitle || '未选择卷' }}</span>
        </div>
        <div class="word-count">
          <el-icon><Collection /></el-icon>
          <span>{{ wordCount.toLocaleString() }} 字</span>
        </div>
      </div>
      <div class="toolbar-right">
        <!-- AI 摘要按钮 -->
        <el-button
          v-if="bookStore.currentChapter"
          type="warning"
          plain
          size="small"
          :icon="Document"
          :loading="isGeneratingSummary"
          @click="handleGenerateSummary"
          class="action-btn"
        >
          生成摘要
        </el-button>
        <el-button
          v-if="chapterSummary"
          type="info"
          plain
          size="small"
          :icon="View"
          @click="toggleSummaryPanel"
          class="action-btn"
        >
          查看摘要
        </el-button>
        <el-divider direction="vertical" class="toolbar-divider" />
        <!-- 保存状态 -->
        <div class="save-status" :class="saveStatus">
          <el-icon v-if="saveStatus === 'saved'"><Check /></el-icon>
          <el-icon v-else-if="saveStatus === 'saving'" class="is-loading"><Loading /></el-icon>
          <el-icon v-else><Warning /></el-icon>
          <span>{{ saveStatus === 'saved' ? '已保存' : saveStatus === 'saving' ? '保存中...' : '未保存' }}</span>
        </div>
        <el-button
          type="primary"
          :icon="Check"
          circle
          size="small"
          @click="performSave"
          title="保存 (Ctrl+S)"
          class="save-btn"
        />
      </div>
    </header>

    <!-- 编辑区域 -->
    <div class="editor-container">
      <template v-if="bookStore.currentChapter">
        <div class="editor-paper" v-loading="isLoading">
          <!-- 章节标题 -->
          <div class="chapter-header">
            <div class="title-composer">
              <span class="composer-text">第</span>
              <input
                v-model="chapterNumber"
                class="composer-input number"
                placeholder=""
                @blur="saveChapterTitle"
              />
              <span class="composer-text">章</span>
              <input
                v-model="chapterName"
                class="composer-input name"
                placeholder="请输入章节标题"
                @blur="saveChapterTitle"
                @keyup.enter="saveChapterTitle"
              />
            </div>
          </div>
          
          <!-- 编辑区域 -->
          <div
            ref="editorRef"
            class="editor-content"
            contenteditable="true"
            @input="handleInput"
            @keydown="handleKeyDown"
            :style="{
              fontSize: configStore.config.fontSize + 'px',
              lineHeight: configStore.config.lineHeight
            }"
          />
        </div>
        
        <!-- 润色工具栏 -->
        <transition name="fade">
          <div
            v-if="showPolishToolbar && !isPolishing"
            class="polish-toolbar"
            :style="{
              left: polishToolbarPos.x + 'px',
              top: polishToolbarPos.y + 'px'
            }"
          >
            <el-button
              type="primary"
              size="small"
              :icon="MagicStick"
              @click="polishText"
            >
              AI 润色
            </el-button>
          </div>
        </transition>
      </template>
      
      <!-- 空状态 -->
      <div v-else class="empty-state">
        <div class="empty-icon">✍️</div>
        <h3 class="empty-title">开始创作</h3>
        <p class="empty-desc">请在左侧栏选择或创建一个章节</p>
      </div>
    </div>

    <!-- 摘要面板 -->
    <el-drawer
      v-model="showSummaryPanel"
      title="章节摘要"
      direction="rtl"
      size="380px"
      class="summary-drawer"
    >
      <div v-if="chapterSummary" class="summary-content">
        <div class="summary-card">
          <label class="summary-label">一句话摘要</label>
          <p class="summary-text">{{ chapterSummary.shortSummary }}</p>
        </div>
        
        <div class="summary-card">
          <label class="summary-label">详细摘要</label>
          <p class="summary-text long">{{ chapterSummary.longSummary }}</p>
        </div>
        
        <div v-if="chapterSummary.tags.length > 0" class="summary-card">
          <label class="summary-label">标签</label>
          <div class="tag-cloud">
            <el-tag v-for="tag in chapterSummary.tags" :key="tag" size="small" effect="plain" round>
              {{ tag }}
            </el-tag>
          </div>
        </div>
        
        <div v-if="chapterSummary.characters.length > 0" class="summary-card">
          <label class="summary-label">出场角色</label>
          <div class="tag-cloud">
            <el-tag v-for="char in chapterSummary.characters" :key="char" size="small" type="success" effect="light" round>
              {{ char }}
            </el-tag>
          </div>
        </div>
        
        <div v-if="chapterSummary.locations.length > 0" class="summary-card">
          <label class="summary-label">场景地点</label>
          <div class="tag-cloud">
            <el-tag v-for="loc in chapterSummary.locations" :key="loc" size="small" type="warning" effect="light" round>
              {{ loc }}
            </el-tag>
          </div>
        </div>
        
        <div v-if="chapterSummary.events.length > 0" class="summary-card">
          <label class="summary-label">关键事件</label>
          <ul class="event-list">
            <li v-for="event in chapterSummary.events" :key="event">{{ event }}</li>
          </ul>
        </div>

        <!-- 新增：剧情推进点 -->
        <div v-if="chapterSummary.plotProgression" class="summary-card">
          <label class="summary-label">剧情推进</label>
          <p class="summary-text">{{ chapterSummary.plotProgression }}</p>
        </div>

        <!-- 新增：情感节点 -->
        <div v-if="chapterSummary.emotionalBeats && chapterSummary.emotionalBeats.length > 0" class="summary-card">
          <label class="summary-label">情感节点</label>
          <ul class="event-list">
            <li v-for="beat in chapterSummary.emotionalBeats" :key="beat">{{ beat }}</li>
          </ul>
        </div>

        <!-- 新增：伏笔 -->
        <div v-if="chapterSummary.foreshadowing && chapterSummary.foreshadowing.length > 0" class="summary-card">
          <label class="summary-label">埋下伏笔</label>
          <ul class="event-list">
            <li v-for="item in chapterSummary.foreshadowing" :key="item">{{ item }}</li>
          </ul>
        </div>

        <!-- 新增：未解决线索 -->
        <div v-if="chapterSummary.unresolvedThreads && chapterSummary.unresolvedThreads.length > 0" class="summary-card">
          <label class="summary-label">未解决线索</label>
          <ul class="event-list">
            <li v-for="thread in chapterSummary.unresolvedThreads" :key="thread">{{ thread }}</li>
          </ul>
        </div>
      </div>
      
      <div v-else class="summary-empty">
        <el-icon :size="48" color="var(--nw-text-muted)"><Document /></el-icon>
        <p>暂无摘要，点击"生成摘要"按钮创建</p>
      </div>
    </el-drawer>
  </div>
</template>

<style scoped>
.writing-area {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--nw-bg-page);
}

/* 顶部工具栏 */
.editor-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--nw-space) var(--nw-space-lg);
  background: var(--nw-bg-primary);
  border-bottom: 1px solid var(--nw-border-light);
  flex-shrink: 0;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: var(--nw-space-md);
}

.breadcrumb {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--nw-text-secondary);
  font-size: 14px;
}

.breadcrumb-icon {
  color: var(--nw-primary);
}

.breadcrumb-text {
  font-weight: 500;
}

.word-count {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--nw-text-tertiary);
  font-size: 13px;
  font-family: var(--nw-font-mono);
}

.word-count .el-icon {
  color: var(--nw-accent);
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: var(--nw-space-sm);
}

.action-btn {
  font-size: 13px;
}

.toolbar-divider {
  margin: 0 var(--nw-space-sm);
}

.save-status {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  padding: 4px 10px;
  border-radius: var(--nw-radius-sm);
  transition: all var(--nw-transition-fast);
}

.save-status.saved {
  color: var(--nw-success);
  background: rgba(39, 174, 96, 0.1);
}

.save-status.saving {
  color: var(--nw-warning);
  background: rgba(212, 165, 116, 0.1);
}

.save-status.unsaved {
  color: var(--nw-danger);
  background: rgba(231, 76, 60, 0.1);
}

.save-btn {
  margin-left: var(--nw-space-sm);
}

/* 编辑区域容器 */
.editor-container {
  flex: 1;
  overflow: hidden;
  padding: var(--nw-space-lg);
  background: var(--nw-bg-page);
}

/* 纸张效果 */
.editor-paper {
  height: 100%;
  margin: 0 auto;
  background: var(--nw-bg-primary);
  border-radius: var(--nw-radius);
  box-shadow: var(--nw-shadow);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* 章节标题 */
.chapter-header {
  padding: var(--nw-space-xl) var(--nw-space-xl) var(--nw-space-lg);
  border-bottom: 1px solid var(--nw-border-light);
  flex-shrink: 0;
}

.title-composer {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: var(--nw-space-sm);
}

.composer-text {
  font-size: 20px;
  color: var(--nw-text-secondary);
  font-family: var(--nw-font-display);
}

.composer-input {
  background: transparent;
  border: none;
  border-bottom: 2px solid transparent;
  padding: 4px 8px;
  font-size: 20px;
  font-family: var(--nw-font-display);
  color: var(--nw-text-primary);
  text-align: left;
  transition: all var(--nw-transition-fast);
  outline: none;
}

.composer-input:hover {
  border-bottom-color: var(--nw-border);
}

.composer-input:focus {
  border-bottom-color: var(--nw-primary);
}

.composer-input.number {
  width: 60px;
  text-align: center;
}

.composer-input.name {
  flex: 1;
  max-width: 400px;
}

.composer-input::placeholder {
  color: var(--nw-text-muted);
}

/* 编辑器内容 */
.editor-content {
  flex: 1;
  padding: var(--nw-space-xl);
  outline: none;
  color: var(--nw-text-primary);
  text-indent: 2em;
  overflow-y: auto;
  line-height: 1.8;
  font-family: var(--nw-font-body);
}

.editor-content p {
  margin: 0 0 1em 0;
  min-height: 1.8em;
}

.editor-content p:last-child {
  margin-bottom: 0;
}

.editor-content p:empty::before {
  content: '\200B';
  display: inline;
}

/* 润色工具栏 */
.polish-toolbar {
  position: fixed;
  transform: translateX(-50%);
  z-index: 1000;
  background: var(--nw-bg-primary);
  border-radius: var(--nw-radius);
  box-shadow: var(--nw-shadow-md);
  padding: var(--nw-space-sm);
  border: 1px solid var(--nw-border);
}

.polish-toolbar::after {
  content: '';
  position: absolute;
  bottom: -8px;
  left: 50%;
  transform: translateX(-50%);
  border-left: 8px solid transparent;
  border-right: 8px solid transparent;
  border-top: 8px solid var(--nw-bg-primary);
}

/* 淡入淡出动画 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(-10px);
}

/* 空状态 */
.empty-state {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  color: var(--nw-text-secondary);
}

.empty-icon {
  font-size: 64px;
  margin-bottom: var(--nw-space-md);
  opacity: 0.8;
}

.empty-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--nw-text-primary);
  margin: 0 0 var(--nw-space-sm) 0;
  font-family: var(--nw-font-display);
}

.empty-desc {
  font-size: 14px;
  color: var(--nw-text-tertiary);
  margin: 0;
}

/* 摘要抽屉 */
.summary-drawer :deep(.el-drawer__header) {
  margin-bottom: 0;
  padding: var(--nw-space-md) var(--nw-space-lg);
  border-bottom: 1px solid var(--nw-border-light);
}

.summary-drawer :deep(.el-drawer__body) {
  padding: 0;
  overflow-y: auto;
  background: var(--nw-bg-secondary);
}

.summary-content {
  padding: var(--nw-space-lg);
}

.summary-card {
  background: var(--nw-bg-primary);
  border-radius: var(--nw-radius);
  padding: var(--nw-space-md);
  margin-bottom: var(--nw-space-md);
  box-shadow: var(--nw-shadow-sm);
}

.summary-card:last-child {
  margin-bottom: 0;
}

.summary-label {
  display: block;
  font-size: 12px;
  font-weight: 600;
  color: var(--nw-text-tertiary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: var(--nw-space-sm);
}

.summary-text {
  margin: 0;
  font-size: 14px;
  line-height: 1.8;
  color: var(--nw-text-primary);
}

.summary-text.long {
  color: var(--nw-text-secondary);
}

.tag-cloud {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.event-list {
  margin: 0;
  padding-left: 20px;
}

.event-list li {
  font-size: 14px;
  line-height: 1.8;
  color: var(--nw-text-primary);
  margin-bottom: 6px;
  padding-left: 4px;
}

.event-list li::marker {
  color: var(--nw-accent);
}

.event-list li:last-child {
  margin-bottom: 0;
}

.summary-empty {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  color: var(--nw-text-tertiary);
  padding: var(--nw-space-xl);
}

.summary-empty p {
  margin-top: var(--nw-space);
  font-size: 14px;
}

/* 滚动条 */
.editor-content::-webkit-scrollbar,
.summary-drawer :deep(.el-drawer__body)::-webkit-scrollbar {
  width: 6px;
}

.editor-content::-webkit-scrollbar-track,
.summary-drawer :deep(.el-drawer__body)::-webkit-scrollbar-track {
  background: transparent;
}

.editor-content::-webkit-scrollbar-thumb,
.summary-drawer :deep(.el-drawer__body)::-webkit-scrollbar-thumb {
  background: var(--nw-border);
  border-radius: 3px;
}

.editor-content::-webkit-scrollbar-thumb:hover,
.summary-drawer :deep(.el-drawer__body)::-webkit-scrollbar-thumb:hover {
  background: var(--nw-text-muted);
}
</style>
