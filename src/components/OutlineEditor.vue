<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, onActivated } from 'vue';
import { useBookStore } from '../stores/book';
import { ElMessage } from 'element-plus';
import type { Outline } from '../types';
import {
  Document,
  Collection,
  Reading,
  ArrowRight,
  Check
} from '@element-plus/icons-vue';

const props = defineProps<{
  modelValue: boolean;
  // 当前编辑的章节信息
  currentChapterId?: string;
  currentVolumeId?: string;
}>();

defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const bookStore = useBookStore();
const loading = ref(false);
const saving = ref(false);

// 大纲数据
const bookCoarseOutline = ref<Outline | null>(null);
const bookFineOutline = ref<Outline | null>(null);
const volumeCoarseOutline = ref<Outline | null>(null);
const volumeFineOutline = ref<Outline | null>(null);
const chapterCoarseOutline = ref<Outline | null>(null);
const chapterFineOutline = ref<Outline | null>(null);

// 编辑状态
const editingType = ref<'coarse' | 'fine'>('coarse');
const editingLevel = ref<'book' | 'volume' | 'chapter'>('chapter');
const editContent = ref('');
const isEditing = ref(false);

// 当前卷和章节信息
const currentVolume = computed(() => {
  if (!props.currentVolumeId || !bookStore.currentBook) return null;
  return bookStore.currentBook.volumes.find(v => v.id === props.currentVolumeId);
});

const currentChapter = computed(() => {
  if (!props.currentChapterId || !bookStore.currentBook) return null;
  return bookStore.currentBook.chapters.find(c => c.id === props.currentChapterId) || null;
});

// 面包屑路径
const breadcrumbPath = computed(() => {
  const path = [];
  if (bookStore.currentBook) {
    path.push({ name: bookStore.currentBook.title, type: 'book' });
  }
  if (currentVolume.value) {
    path.push({ name: currentVolume.value.title, type: 'volume' });
  }
  if (currentChapter.value) {
    path.push({ name: currentChapter.value.title, type: 'chapter' });
  }
  return path;
});

// 获取当前编辑的大纲
const getCurrentEditingOutline = (): Outline | null => {
  if (editingLevel.value === 'book') {
    return editingType.value === 'coarse' ? bookCoarseOutline.value : bookFineOutline.value;
  } else if (editingLevel.value === 'volume') {
    return editingType.value === 'coarse' ? volumeCoarseOutline.value : volumeFineOutline.value;
  } else {
    return editingType.value === 'coarse' ? chapterCoarseOutline.value : chapterFineOutline.value;
  }
};

// 加载所有大纲
const loadAllOutlines = async () => {
  if (!bookStore.currentBook) return;
  loading.value = true;
  
  try {
    // 加载书籍级大纲
    const bookOutlines = await bookStore.listBookOutlines();
    bookCoarseOutline.value = bookOutlines.find(o => o.outline_type === 'coarse') || null;
    bookFineOutline.value = bookOutlines.find(o => o.outline_type === 'fine') || null;

    // 加载卷级大纲
    if (props.currentVolumeId) {
      const volumeOutlines = await bookStore.listVolumeOutlines(props.currentVolumeId);
      volumeCoarseOutline.value = volumeOutlines.find(o => o.outline_type === 'coarse') || null;
      volumeFineOutline.value = volumeOutlines.find(o => o.outline_type === 'fine') || null;
    }

    // 加载章节级大纲
    if (props.currentChapterId) {
      const chapterOutlines = await bookStore.listChapterOutlines(props.currentChapterId);
      chapterCoarseOutline.value = chapterOutlines.find(o => o.outline_type === 'coarse') || null;
      chapterFineOutline.value = chapterOutlines.find(o => o.outline_type === 'fine') || null;
    }
  } catch (error) {
    console.error('加载大纲失败:', error);
    ElMessage.error('加载大纲失败');
  } finally {
    loading.value = false;
  }
};

// 开始编辑
const startEditing = (level: 'book' | 'volume' | 'chapter', type: 'coarse' | 'fine') => {
  editingLevel.value = level;
  editingType.value = type;
  
  const outline = getCurrentEditingOutline();
  editContent.value = outline?.content || '';
  isEditing.value = true;
};

// 保存大纲
const saveOutline = async () => {
  if (!bookStore.currentBook) return;
  
  saving.value = true;
  try {
    let volumeId: string | undefined;
    let chapterId: string | undefined;
    
    if (editingLevel.value === 'volume') {
      volumeId = props.currentVolumeId;
    } else if (editingLevel.value === 'chapter') {
      volumeId = props.currentVolumeId;
      chapterId = props.currentChapterId;
    }
    
    await bookStore.saveOutline({
      book_id: bookStore.currentBook.id,
      volume_id: volumeId,
      chapter_id: chapterId,
      outline_type: editingType.value,
      content: editContent.value
    });
    
    ElMessage.success('大纲保存成功');
    isEditing.value = false;
    await loadAllOutlines();
  } catch (error) {
    console.error('保存大纲失败:', error);
    ElMessage.error('保存大纲失败');
  } finally {
    saving.value = false;
  }
};

// 取消编辑
const cancelEditing = () => {
  isEditing.value = false;
  editContent.value = '';
};

// 获取大纲摘要
const getOutlineSummary = (outline: Outline | null): string => {
  if (!outline || !outline.content) return '暂无内容';
  const lines = outline.content.split('\n').filter(l => l.trim());
  if (lines.length === 0) return '暂无内容';
  return lines[0].slice(0, 50) + (lines[0].length > 50 ? '...' : '');
};

// 是否有大纲内容
const hasOutline = (outline: Outline | null): boolean => {
  return !!outline && !!outline.content.trim();
};

// 监听大纲保存事件
const handleOutlineSaved = (event: CustomEvent<{ chapterId: string; outlineType: string }>) => {
  const { chapterId } = event.detail;

  // 如果保存的是当前章节的大纲
  if (chapterId === props.currentChapterId) {
    if (props.modelValue) {
      // 如果编辑器已打开，立即刷新
      loadAllOutlines();

      // 如果在编辑模式，也刷新编辑内容
      if (isEditing.value) {
        const outline = getCurrentEditingOutline();
        if (outline) {
          editContent.value = outline.content || '';
        }
      }
    } else {
      // 如果编辑器未打开，设置标记，下次打开时刷新
      needsRefresh.value = true;
    }
  }
};

// 标记是否需要刷新
const needsRefresh = ref(false);

watch(() => props.modelValue, (visible) => {
  if (visible) {
    // 如果有刷新标记或需要加载数据，则刷新
    if (needsRefresh.value || !bookCoarseOutline.value) {
      console.log('[OutlineEditor] 打开编辑器，刷新数据');
      loadAllOutlines();
      needsRefresh.value = false;
    }
  }
});

onMounted(() => {
  if (props.modelValue) {
    loadAllOutlines();
  }
  // 监听大纲保存事件
  window.addEventListener('outline-saved', handleOutlineSaved as EventListener);
});

onUnmounted(() => {
  // 移除事件监听
  window.removeEventListener('outline-saved', handleOutlineSaved as EventListener);
});

// 组件激活时（从缓存中恢复）也刷新
onActivated(() => {
  if (props.modelValue) {
    loadAllOutlines();
  }
});
</script>

<template>
  <el-drawer
    :model-value="modelValue"
    @update:model-value="$emit('update:modelValue', $event)"
    title="大纲管理"
    direction="rtl"
    size="550px"
    :close-on-click-modal="false"
    class="outline-drawer"
  >
    <div class="outline-manager">
      <!-- 面包屑导航 -->
      <div class="breadcrumb">
        <el-breadcrumb separator="/">
          <el-breadcrumb-item v-for="(item, index) in breadcrumbPath" :key="index">
            {{ item.name }}
          </el-breadcrumb-item>
        </el-breadcrumb>
      </div>

      <!-- 编辑模式 -->
      <div v-if="isEditing" class="edit-mode">
        <div class="edit-header">
          <el-button text :icon="ArrowRight" @click="cancelEditing">返回</el-button>
          <span class="edit-title">
            {{ editingLevel === 'book' ? '书籍' : editingLevel === 'volume' ? '卷' : '章节' }}的
            {{ editingType === 'coarse' ? '粗纲' : '细纲' }}
          </span>
          <el-button type="primary" :icon="Check" :loading="saving" @click="saveOutline">
            保存
          </el-button>
        </div>
        
        <el-input
          v-model="editContent"
          type="textarea"
          :rows="20"
          :placeholder="editingType === 'coarse' ? '编写粗纲...' : '编写细纲...'"
          class="outline-textarea"
        />
      </div>

      <!-- 浏览模式 -->
      <div v-else v-loading="loading" class="outline-list">
        <!-- 书籍级大纲 -->
        <div class="outline-section">
          <div class="section-title">
            <el-icon><Reading /></el-icon>
            <span>书籍大纲</span>
          </div>
          
          <div class="outline-cards">
            <div 
              class="outline-card"
              :class="{ 'has-content': hasOutline(bookCoarseOutline) }"
              @click="startEditing('book', 'coarse')"
            >
              <div class="card-type">粗纲</div>
              <div class="card-content">{{ getOutlineSummary(bookCoarseOutline) }}</div>
              <div class="card-status">
                <el-tag v-if="hasOutline(bookCoarseOutline)" type="success" size="small">已编写</el-tag>
                <el-tag v-else type="info" size="small">未编写</el-tag>
              </div>
            </div>
            
            <div 
              class="outline-card"
              :class="{ 'has-content': hasOutline(bookFineOutline) }"
              @click="startEditing('book', 'fine')"
            >
              <div class="card-type">细纲</div>
              <div class="card-content">{{ getOutlineSummary(bookFineOutline) }}</div>
              <div class="card-status">
                <el-tag v-if="hasOutline(bookFineOutline)" type="success" size="small">已编写</el-tag>
                <el-tag v-else type="info" size="small">未编写</el-tag>
              </div>
            </div>
          </div>
        </div>

        <!-- 卷级大纲 -->
        <div v-if="currentVolume" class="outline-section">
          <div class="section-title">
            <el-icon><Collection /></el-icon>
            <span>卷大纲 - {{ currentVolume.title }}</span>
          </div>
          
          <div class="outline-cards">
            <div 
              class="outline-card"
              :class="{ 'has-content': hasOutline(volumeCoarseOutline) }"
              @click="startEditing('volume', 'coarse')"
            >
              <div class="card-type">粗纲</div>
              <div class="card-content">{{ getOutlineSummary(volumeCoarseOutline) }}</div>
              <div class="card-status">
                <el-tag v-if="hasOutline(volumeCoarseOutline)" type="success" size="small">已编写</el-tag>
                <el-tag v-else type="info" size="small">未编写</el-tag>
              </div>
            </div>
            
            <div 
              class="outline-card"
              :class="{ 'has-content': hasOutline(volumeFineOutline) }"
              @click="startEditing('volume', 'fine')"
            >
              <div class="card-type">细纲</div>
              <div class="card-content">{{ getOutlineSummary(volumeFineOutline) }}</div>
              <div class="card-status">
                <el-tag v-if="hasOutline(volumeFineOutline)" type="success" size="small">已编写</el-tag>
                <el-tag v-else type="info" size="small">未编写</el-tag>
              </div>
            </div>
          </div>
        </div>

        <!-- 章节级大纲 -->
        <div v-if="currentChapter" class="outline-section">
          <div class="section-title">
            <el-icon><Document /></el-icon>
            <span>章节大纲 - {{ currentChapter.title }}</span>
          </div>
          
          <div class="outline-cards">
            <div 
              class="outline-card"
              :class="{ 'has-content': hasOutline(chapterCoarseOutline) }"
              @click="startEditing('chapter', 'coarse')"
            >
              <div class="card-type">粗纲</div>
              <div class="card-content">{{ getOutlineSummary(chapterCoarseOutline) }}</div>
              <div class="card-status">
                <el-tag v-if="hasOutline(chapterCoarseOutline)" type="success" size="small">已编写</el-tag>
                <el-tag v-else type="info" size="small">未编写</el-tag>
              </div>
            </div>
            
            <div 
              class="outline-card"
              :class="{ 'has-content': hasOutline(chapterFineOutline) }"
              @click="startEditing('chapter', 'fine')"
            >
              <div class="card-type">细纲</div>
              <div class="card-content">{{ getOutlineSummary(chapterFineOutline) }}</div>
              <div class="card-status">
                <el-tag v-if="hasOutline(chapterFineOutline)" type="success" size="small">已编写</el-tag>
                <el-tag v-else type="info" size="small">未编写</el-tag>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </el-drawer>
</template>

<style scoped>
.outline-drawer :deep(.el-drawer__body) {
  padding: 0;
}

.outline-manager {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.breadcrumb {
  padding: 16px;
  border-bottom: 1px solid var(--el-border-color-light);
}

/* 编辑模式 */
.edit-mode {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.edit-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--el-border-color-light);
}

.edit-title {
  font-weight: 500;
  color: var(--el-text-color-primary);
}

.outline-textarea {
  flex: 1;
  padding: 16px;
}

.outline-textarea :deep(.el-textarea__inner) {
  height: 100%;
  resize: none;
  font-size: 14px;
  line-height: 1.8;
}

/* 浏览模式 */
.outline-list {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.outline-section {
  margin-bottom: 24px;
}

.section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
  font-size: 15px;
  color: var(--el-text-color-primary);
  margin-bottom: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--el-border-color-light);
}

.section-title .el-icon {
  color: var(--el-color-primary);
}

.outline-cards {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.outline-card {
  padding: 16px;
  border: 2px solid var(--el-border-color-light);
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.outline-card:hover {
  border-color: var(--el-color-primary-light-5);
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.05);
  transform: translateY(-1px);
}

.outline-card.has-content {
  border-color: var(--el-color-success-light-5);
}

.outline-card.has-content:hover {
  border-color: var(--el-color-success);
}

.card-type {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.card-content {
  font-size: 13px;
  color: var(--el-text-color-regular);
  line-height: 1.5;
  min-height: 40px;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.card-status {
  margin-top: auto;
}

@media (max-width: 768px) {
  .outline-cards {
    grid-template-columns: 1fr;
  }
}
</style>
