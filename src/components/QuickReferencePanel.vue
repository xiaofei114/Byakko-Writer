<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';
import { useBookStore } from '../stores/book';
import type { CharacterCard, Outline } from '../types';
import {
  User,
  ArrowRight
} from '@element-plus/icons-vue';
import { marked } from 'marked';

marked.setOptions({
  breaks: true,
  gfm: true
});

const props = defineProps<{
  modelValue: boolean;
  type: 'character' | 'outline';
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
  (e: 'openFullManager'): void;
}>();

const bookStore = useBookStore();
const loading = ref(false);

const renderMarkdown = (content: string) => {
  if (!content) return '';
  return marked.parse(content);
};

// 角色卡数据
const characterCards = ref<CharacterCard[]>([]);
const selectedCharacter = ref<CharacterCard | null>(null);

// 大纲数据
const coarseOutline = ref<Outline | null>(null);
const fineOutline = ref<Outline | null>(null);
const hasCoarse = computed(() => !!coarseOutline.value?.content);
const hasFine = computed(() => !!fineOutline.value?.content);

// 加载角色卡列表
const loadCharacterCards = async () => {
  if (!bookStore.currentBook || props.type !== 'character') return;
  loading.value = true;
  try {
    const cards = await bookStore.listCharacterCards();
    characterCards.value = cards;
  } catch (error) {
    console.error('加载角色卡失败:', error);
  } finally {
    loading.value = false;
  }
};

// 加载当前章节大纲
const loadCurrentOutline = async () => {
  if (!bookStore.currentBook || props.type !== 'outline') return;
  loading.value = true;
  try {
    const [coarse, fine] = await Promise.all([
      bookStore.getOutlineByLevel(undefined, bookStore.currentChapterId || undefined, 'coarse'),
      bookStore.getOutlineByLevel(undefined, bookStore.currentChapterId || undefined, 'fine'),
    ]);
    coarseOutline.value = coarse;
    fineOutline.value = fine;
  } catch (error) {
    console.error('加载大纲失败:', error);
  } finally {
    loading.value = false;
  }
};

// 选择角色卡
const selectCharacter = (card: CharacterCard) => {
  selectedCharacter.value = card;
};

// 返回角色卡列表
const backToCharacterList = () => {
  selectedCharacter.value = null;
};

// 打开完整管理器
const openFullManager = () => {
  emit('openFullManager');
  emit('update:modelValue', false);
};

// 监听显示状态
watch(() => props.modelValue, (visible) => {
  if (visible) {
    if (props.type === 'character') {
      loadCharacterCards();
    } else {
      loadCurrentOutline();
    }
  }
});

// 监听当前章节变化
watch(() => bookStore.currentChapterId, () => {
  if (props.modelValue && props.type === 'outline') {
    loadCurrentOutline();
  }
});

onMounted(() => {
  if (props.modelValue) {
    if (props.type === 'character') {
      loadCharacterCards();
    } else {
      loadCurrentOutline();
    }
  }
});
</script>

<template>
  <el-drawer
    :model-value="modelValue"
    @update:model-value="$emit('update:modelValue', $event)"
    :title="type === 'character' ? '角色卡' : '本章大纲'"
    direction="rtl"
    size="500px"
    :close-on-click-modal="true"
    :class="['quick-reference-drawer', `drawer-${type}`]"
  >
    <div class="quick-reference-panel">
      <!-- 角色卡快速查看 -->
      <template v-if="type === 'character'">
        <!-- 角色卡列表 -->
        <div v-if="!selectedCharacter" v-loading="loading" class="quick-list">
          <div
            v-for="card in characterCards"
            :key="card.id"
            class="quick-item"
            @click="selectCharacter(card)"
          >
            <div class="item-avatar">
              <el-icon><User /></el-icon>
            </div>
            <div class="item-info">
              <div class="item-name">{{ card.name }}</div>
              <div class="item-meta">
                <span v-if="card.gender">{{ card.gender }}</span>
                <span v-if="card.age">{{ card.age }}</span>
              </div>
            </div>
            <el-icon class="item-arrow"><ArrowRight /></el-icon>
          </div>
          
          <el-empty v-if="characterCards.length === 0" description="暂无角色卡" />
          
          <el-button 
            text 
            type="primary" 
            class="open-full-btn"
            @click="openFullManager"
          >
            打开完整角色卡管理
          </el-button>
        </div>
        
        <!-- 角色卡详情 -->
        <div v-else class="quick-detail">
          <div class="detail-header">
            <el-button text :icon="ArrowRight" @click="backToCharacterList">返回</el-button>
          </div>
          
          <div class="character-info">
            <div class="info-avatar">
              <el-icon><User /></el-icon>
            </div>
            <div class="info-title">
              <h4>{{ selectedCharacter.name }}</h4>
              <div class="info-tags">
                <span v-if="selectedCharacter.gender" class="info-tag">{{ selectedCharacter.gender }}</span>
                <span v-if="selectedCharacter.age" class="info-tag">{{ selectedCharacter.age }}</span>
              </div>
            </div>
          </div>
          
          <div class="info-sections">
            <div v-if="selectedCharacter.appearance" class="info-section">
              <div class="section-title">外貌</div>
              <div class="section-content">{{ selectedCharacter.appearance }}</div>
            </div>
            
            <div v-if="selectedCharacter.personality" class="info-section">
              <div class="section-title">性格</div>
              <div class="section-content">{{ selectedCharacter.personality }}</div>
            </div>
            
            <div v-if="selectedCharacter.background" class="info-section">
              <div class="section-title">背景</div>
              <div class="section-content">{{ selectedCharacter.background }}</div>
            </div>
            
            <div v-if="selectedCharacter.goals" class="info-section">
              <div class="section-title">目标</div>
              <div class="section-content">{{ selectedCharacter.goals }}</div>
            </div>
            
            <div v-if="selectedCharacter.relationships.length > 0" class="info-section">
              <div class="section-title">关系</div>
              <div class="section-content">
                <div v-for="(rel, index) in selectedCharacter.relationships" :key="index" class="relation-item">
                  <span class="rel-name">{{ rel.target_name }}</span>
                  <span class="rel-type">{{ rel.relationship }}</span>
                </div>
              </div>
            </div>
            
            <div v-if="selectedCharacter.notes" class="info-section">
              <div class="section-title">备注</div>
              <div class="section-content">{{ selectedCharacter.notes }}</div>
            </div>
          </div>
          
          <el-button 
            text 
            type="primary" 
            class="open-full-btn"
            @click="openFullManager"
          >
            打开完整角色卡管理
          </el-button>
        </div>
      </template>
      
      <!-- 大纲快速查看 -->
      <template v-else>
        <div v-loading="loading" class="outline-view">
          <div v-if="hasCoarse || hasFine" class="outline-content">
            <div
                v-if="hasCoarse"
                class="outline-section"
              >
                <div class="outline-section-header">
                  <span class="outline-section-dot coarse-dot"></span>
                  粗纲
                </div>
                <div
                  class="outline-preview markdown-body"
                  v-html="renderMarkdown(coarseOutline!.content)"
                ></div>
              </div>
              <div
                v-if="hasFine"
                class="outline-section"
              >
                <div class="outline-section-header">
                  <span class="outline-section-dot fine-dot"></span>
                  细纲
                </div>
                <div
                  class="outline-preview markdown-body"
                  v-html="renderMarkdown(fineOutline!.content)"
                ></div>
              </div>
          </div>
          <el-empty v-else description="当前章节暂无大纲" />
        </div>
      </template>
    </div>
  </el-drawer>
</template>

<style scoped>
.quick-reference-drawer :deep(.el-drawer__body) {
  padding: 0;
}

.quick-reference-drawer.drawer-outline {
  :deep(.el-drawer) {
    width: 500px !important;
  }
}

.quick-reference-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
}

/* 列表样式 */
.quick-list {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.quick-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.quick-item:hover {
  background: var(--el-fill-color-light);
}

.item-avatar {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background: var(--el-color-primary-light-9);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--el-color-primary);
  font-size: 18px;
}

.item-info {
  flex: 1;
  min-width: 0;
}

.item-name {
  font-weight: 500;
  font-size: 14px;
  color: var(--el-text-color-primary);
  margin-bottom: 2px;
}

.item-meta {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  display: flex;
  gap: 8px;
}

.item-arrow {
  color: var(--el-text-color-secondary);
}

/* 详情样式 */
.quick-detail {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.detail-header {
  padding: 12px;
  border-bottom: 1px solid var(--el-border-color-light);
}

.character-info {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 20px 16px;
  border-bottom: 1px solid var(--el-border-color-light);
}

.info-avatar {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  background: var(--el-color-primary-light-9);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--el-color-primary);
  font-size: 28px;
}

.info-title h4 {
  margin: 0 0 8px 0;
  font-size: 18px;
}

.info-tags {
  display: flex;
  gap: 8px;
}

.info-tag {
  background: var(--el-fill-color-light);
  padding: 2px 10px;
  border-radius: 12px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.info-sections {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.info-section {
  margin-bottom: 16px;
}

.section-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 6px;
}

.section-content {
  font-size: 13px;
  color: var(--el-text-color-primary);
  line-height: 1.6;
}

.relation-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
}

.rel-name {
  font-weight: 500;
  color: var(--el-color-primary);
}

.rel-type {
  background: var(--el-fill-color-light);
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 11px;
}

/* 大纲样式 */
.outline-view {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 20px 20px 0;
  overflow: hidden;
}

.outline-content {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 24px;
  padding-bottom: 20px;
}

.outline-section {
  flex-shrink: 0;
}

.outline-section:only-child {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.outline-section:only-child .outline-preview {
  flex: 1;
}

.outline-section-header {
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  margin-bottom: 10px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.outline-section-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}

.coarse-dot {
  background: var(--el-color-primary);
}

.fine-dot {
  background: var(--el-color-success);
}

.outline-preview {
  font-size: 15px;
  line-height: 1.9;
  color: var(--el-text-color-primary);
  background: var(--el-fill-color-light);
  padding: 16px 18px;
  border-radius: 8px;
  overflow-x: auto;
}

.outline-preview :deep(h1),
.outline-preview :deep(h2),
.outline-preview :deep(h3),
.outline-preview :deep(h4) {
  margin-top: 0.8em;
  margin-bottom: 0.4em;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.outline-preview :deep(p) {
  margin-bottom: 0.6em;
}

.outline-preview :deep(ul),
.outline-preview :deep(ol) {
  padding-left: 1.5em;
  margin-bottom: 0.6em;
}

.outline-preview :deep(li) {
  margin-bottom: 0.2em;
}

.outline-preview :deep(strong) {
  font-weight: 600;
}

.outline-preview :deep(code) {
  background: var(--el-fill-color-darker);
  padding: 2px 6px;
  border-radius: 3px;
  font-size: 0.9em;
}

.outline-preview :deep(blockquote) {
  border-left: 3px solid var(--el-color-primary-light-5);
  padding-left: 12px;
  margin-left: 0;
  color: var(--el-text-color-secondary);
}

.empty-desc {
  text-align: center;
}

.empty-desc p {
  margin: 4px 0;
}

.empty-hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

/* 通用按钮 */
.open-full-btn {
  margin-top: auto;
  padding: 12px;
  width: 100%;
}
</style>
