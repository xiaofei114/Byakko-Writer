<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue';
import { useBookStore } from '../stores/book';
import { ElMessage, ElMessageBox } from 'element-plus';
import type { CharacterCard, CharacterRelationship } from '../types';
import {
  Plus,
  Search,
  Edit,
  Delete,
  Close,
  Female,
  Male
} from '@element-plus/icons-vue';

const props = defineProps<{
  modelValue: boolean;
}>();

defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const bookStore = useBookStore();
const loading = ref(false);
const characterCards = ref<CharacterCard[]>([]);
const searchKeyword = ref('');

// 编辑对话框
const showEditDialog = ref(false);
const isEditing = ref(false);
const currentCard = ref<Partial<CharacterCard>>({});
const currentRelationships = ref<CharacterRelationship[]>([]);
const currentAliases = ref<string[]>([]);
const currentTags = ref<string[]>([]);

// 详情查看
const showDetailDialog = ref(false);
const viewingCard = ref<CharacterCard | null>(null);

// 过滤后的角色卡
const filteredCards = computed(() => {
  if (!searchKeyword.value.trim()) return characterCards.value;
  const keyword = searchKeyword.value.toLowerCase();
  return characterCards.value.filter(card => 
    card.name.toLowerCase().includes(keyword) ||
    card.tags.some(tag => tag.toLowerCase().includes(keyword)) ||
    card.aliases.some(alias => alias.toLowerCase().includes(keyword))
  );
});

// 获取角色名首字作为头像
const getInitial = (name: string) => {
  return name.charAt(0) || '?';
};

// 根据性别获取颜色
const getGenderColor = (gender: string) => {
  if (gender === '男') return '#409eff';
  if (gender === '女') return '#f56c6c';
  return '#909399';
};

// 加载角色卡列表
const loadCharacterCards = async () => {
  if (!bookStore.currentBook) return;
  loading.value = true;
  try {
    const cards = await bookStore.listCharacterCards();
    characterCards.value = cards;
  } catch (error) {
    console.error('加载角色卡失败:', error);
    ElMessage.error('加载角色卡失败');
  } finally {
    loading.value = false;
  }
};

// 当抽屉打开时重新加载
watch(() => props.modelValue, (visible) => {
  if (visible) {
    loadCharacterCards();
  }
});

// 打开新建对话框
const openCreateDialog = () => {
  isEditing.value = false;
  currentCard.value = {
    book_id: bookStore.currentBook?.id || '',
    name: '',
    gender: '',
    age: '',
    appearance: '',
    personality: '',
    background: '',
    goals: '',
    notes: ''
  };
  currentAliases.value = [];
  currentTags.value = [];
  currentRelationships.value = [];
  showEditDialog.value = true;
};

// 打开编辑对话框
const openEditDialog = (card: CharacterCard) => {
  isEditing.value = true;
  currentCard.value = { ...card };
  currentAliases.value = [...card.aliases];
  currentTags.value = [...card.tags];
  currentRelationships.value = card.relationships.map(r => ({ ...r }));
  showEditDialog.value = true;
};

// 打开详情查看
const openDetailDialog = (card: CharacterCard) => {
  viewingCard.value = card;
  showDetailDialog.value = true;
};

// 保存角色卡
const saveCharacterCard = async () => {
  if (!currentCard.value.name?.trim()) {
    ElMessage.warning('请输入角色名称');
    return;
  }

  try {
    const cardData = {
      ...currentCard.value,
      aliases: currentAliases.value,
      tags: currentTags.value,
      relationships: currentRelationships.value
    } as CharacterCard;

    if (isEditing.value && currentCard.value.id) {
      await bookStore.updateCharacterCard(cardData);
      ElMessage.success('角色卡更新成功');
    } else {
      await bookStore.createCharacterCard(cardData);
      ElMessage.success('角色卡创建成功');
    }
    showEditDialog.value = false;
    await loadCharacterCards();
  } catch (error) {
    console.error('保存角色卡失败:', error);
    ElMessage.error('保存角色卡失败');
  }
};

// 删除角色卡
const deleteCharacterCard = async (card: CharacterCard) => {
  try {
    await ElMessageBox.confirm(
      `确定要删除角色 "${card.name}" 吗？`,
      '确认删除',
      { type: 'warning' }
    );
    await bookStore.deleteCharacterCard(card.id);
    ElMessage.success('角色卡已删除');
    await loadCharacterCards();
  } catch (error) {
    if (error !== 'cancel') {
      console.error('删除角色卡失败:', error);
      ElMessage.error('删除角色卡失败');
    }
  }
};

// 添加关系
const addRelationship = () => {
  currentRelationships.value.push({
    target_character_id: '',
    target_name: '',
    relationship: '',
    description: ''
  } as CharacterRelationship);
};

// 删除关系
const removeRelationship = (index: number) => {
  currentRelationships.value.splice(index, 1);
};

// 添加别名
const addAlias = () => {
  currentAliases.value.push('');
};

// 删除别名
const removeAlias = (index: number) => {
  currentAliases.value.splice(index, 1);
};

// 添加标签
const addTag = () => {
  currentTags.value.push('');
};

// 删除标签
const removeTag = (index: number) => {
  currentTags.value.splice(index, 1);
};

onMounted(() => {
  loadCharacterCards();
});
</script>

<template>
  <el-drawer
    :model-value="modelValue"
    @update:model-value="$emit('update:modelValue', $event)"
    title="角色卡管理"
    direction="rtl"
    size="500px"
    :close-on-click-modal="false"
    class="character-drawer"
  >
    <div class="character-manager">
      <!-- 搜索和操作栏 -->
      <div class="toolbar">
        <el-input
          v-model="searchKeyword"
          placeholder="搜索角色名称、别名或标签"
          :prefix-icon="Search"
          clearable
          class="search-input"
        />
        <el-button type="primary" :icon="Plus" @click="openCreateDialog">
          新建角色
        </el-button>
      </div>

      <!-- 角色卡列表 -->
      <div v-loading="loading" class="character-list">
        <el-empty v-if="filteredCards.length === 0" description="暂无角色卡">
          <template #description>
            <p>暂无角色卡</p>
            <p class="empty-hint">点击上方「新建角色」按钮创建</p>
          </template>
        </el-empty>
        
        <div
          v-for="card in filteredCards"
          :key="card.id"
          class="character-card"
          @click="openDetailDialog(card)"
        >
          <div class="card-header">
            <div
              class="card-avatar"
              :style="{ background: getGenderColor(card.gender) + '18', color: getGenderColor(card.gender) }"
            >
              <span class="avatar-text">{{ getInitial(card.name) }}</span>
            </div>
            <div class="card-info">
              <div class="card-name">{{ card.name }}</div>
              <div class="card-meta">
                <span v-if="card.gender" class="meta-item">
                  <el-icon :size="12" v-if="card.gender === '男'"><Male /></el-icon>
                  <el-icon :size="12" v-else-if="card.gender === '女'"><Female /></el-icon>
                  {{ card.gender }}
                </span>
                <span v-if="card.age" class="meta-item">{{ card.age }}</span>
              </div>
            </div>
            <div class="card-actions" @click.stop>
              <el-button text type="primary" :icon="Edit" @click="openEditDialog(card)" />
              <el-button text type="danger" :icon="Delete" @click="deleteCharacterCard(card)" />
            </div>
          </div>
          
          <div v-if="card.aliases.length > 0" class="card-aliases">
            <span v-for="(alias, i) in card.aliases.slice(0, 2)" :key="i" class="alias-tag">
              {{ alias }}
            </span>
            <span v-if="card.aliases.length > 2" class="more-aliases">
              等{{ card.aliases.length }}个别名
            </span>
          </div>
          
          <div v-if="card.tags.length > 0" class="card-tags">
            <el-tag
              v-for="(tag, index) in card.tags.slice(0, 4)"
              :key="index"
              size="small"
              :hit="false"
              class="card-tag"
            >
              {{ tag }}
            </el-tag>
            <span v-if="card.tags.length > 4" class="more-tags">+{{ card.tags.length - 4 }}</span>
          </div>
          
          <div class="card-preview">
            <span v-if="card.personality" class="preview-text">
              {{ card.personality.slice(0, 60) }}{{ card.personality.length > 60 ? '...' : '' }}
            </span>
            <span v-else class="preview-placeholder">暂无性格描述</span>
          </div>
        </div>
      </div>
    </div>
  </el-drawer>

  <!-- 编辑对话框 -->
  <el-dialog
    v-model="showEditDialog"
    :title="isEditing ? '编辑角色卡' : '新建角色卡'"
    width="600px"
    destroy-on-close
  >
    <el-form label-position="top" class="character-form">
      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="角色名称">
            <el-input v-model="currentCard.name" placeholder="请输入角色名称" />
          </el-form-item>
        </el-col>
        <el-col :span="6">
          <el-form-item label="性别">
            <el-select v-model="currentCard.gender" placeholder="选择" style="width: 100%">
              <el-option label="男" value="男" />
              <el-option label="女" value="女" />
              <el-option label="其他" value="其他" />
            </el-select>
          </el-form-item>
        </el-col>
        <el-col :span="6">
          <el-form-item label="年龄">
            <el-input v-model="currentCard.age" placeholder="如: 25岁" />
          </el-form-item>
        </el-col>
      </el-row>

      <!-- 别名 -->
      <el-form-item label="别名">
        <div class="dynamic-list">
          <div v-for="(_, index) in currentAliases" :key="index" class="dynamic-item">
            <el-input v-model="currentAliases[index]" placeholder="别名" size="small" />
            <el-button text type="danger" :icon="Close" size="small" @click="removeAlias(index)" />
          </div>
          <el-button text type="primary" size="small" @click="addAlias">+ 添加别名</el-button>
        </div>
      </el-form-item>

      <el-form-item label="外貌">
        <el-input
          v-model="currentCard.appearance"
          type="textarea"
          :rows="2"
          placeholder="描述角色的外貌特征"
        />
      </el-form-item>

      <el-form-item label="性格">
        <el-input
          v-model="currentCard.personality"
          type="textarea"
          :rows="2"
          placeholder="描述角色的性格特点"
        />
      </el-form-item>

      <el-form-item label="背景">
        <el-input
          v-model="currentCard.background"
          type="textarea"
          :rows="3"
          placeholder="描述角色的出身背景、经历等"
        />
      </el-form-item>

      <el-form-item label="目标">
        <el-input
          v-model="currentCard.goals"
          type="textarea"
          :rows="2"
          placeholder="描述角色的目标、动机"
        />
      </el-form-item>

      <!-- 关系 -->
      <el-form-item label="人物关系">
        <div class="relationship-list">
          <div v-for="(rel, index) in currentRelationships" :key="index" class="relationship-item">
            <el-input v-model="rel.target_name" placeholder="相关人物" size="small" style="width: 120px" />
            <el-input v-model="rel.relationship" placeholder="关系" size="small" style="width: 100px" />
            <el-input v-model="rel.description" placeholder="描述" size="small" style="flex: 1" />
            <el-button text type="danger" :icon="Close" size="small" @click="removeRelationship(index)" />
          </div>
          <el-button text type="primary" size="small" @click="addRelationship">+ 添加关系</el-button>
        </div>
      </el-form-item>

      <!-- 标签 -->
      <el-form-item label="标签">
        <div class="dynamic-list">
          <div v-for="(_, index) in currentTags" :key="index" class="dynamic-item">
            <el-input v-model="currentTags[index]" placeholder="标签" size="small" />
            <el-button text type="danger" :icon="Close" size="small" @click="removeTag(index)" />
          </div>
          <el-button text type="primary" size="small" @click="addTag">+ 添加标签</el-button>
        </div>
      </el-form-item>

      <el-form-item label="备注">
        <el-input
          v-model="currentCard.notes"
          type="textarea"
          :rows="2"
          placeholder="其他备注信息"
        />
      </el-form-item>
    </el-form>

    <template #footer>
      <el-button @click="showEditDialog = false">取消</el-button>
      <el-button type="primary" @click="saveCharacterCard">保存</el-button>
    </template>
  </el-dialog>

  <!-- 详情查看对话框 -->
  <el-dialog
    v-model="showDetailDialog"
    title="角色详情"
    width="500px"
    destroy-on-close
  >
    <div v-if="viewingCard" class="character-detail">
      <div class="detail-header">
        <div
          class="detail-avatar"
          :style="{ background: getGenderColor(viewingCard.gender) + '18', color: getGenderColor(viewingCard.gender) }"
        >
          <span class="avatar-text" style="font-size: 24px;">{{ getInitial(viewingCard.name) }}</span>
        </div>
        <div class="detail-title">
          <h3>{{ viewingCard.name }}</h3>
          <div class="detail-meta">
            <span
              v-if="viewingCard.gender"
              class="meta-tag"
              :style="{ background: getGenderColor(viewingCard.gender) + '15', color: getGenderColor(viewingCard.gender) }"
            >
              <el-icon :size="12" v-if="viewingCard.gender === '男'"><Male /></el-icon>
              <el-icon :size="12" v-else-if="viewingCard.gender === '女'"><Female /></el-icon>
              {{ viewingCard.gender }}
            </span>
            <span v-if="viewingCard.age" class="meta-tag">{{ viewingCard.age }}</span>
          </div>
        </div>
      </div>

      <div v-if="viewingCard.aliases.length > 0" class="detail-section">
        <div class="section-label">别名</div>
        <div class="section-content">
          <el-tag v-for="(alias, index) in viewingCard.aliases" :key="index" size="small" class="detail-tag">
            {{ alias }}
          </el-tag>
        </div>
      </div>

      <div v-if="viewingCard.appearance" class="detail-section">
        <div class="section-label">外貌</div>
        <div class="section-content text-content">{{ viewingCard.appearance }}</div>
      </div>

      <div v-if="viewingCard.personality" class="detail-section">
        <div class="section-label">性格</div>
        <div class="section-content text-content">{{ viewingCard.personality }}</div>
      </div>

      <div v-if="viewingCard.background" class="detail-section">
        <div class="section-label">背景</div>
        <div class="section-content text-content">{{ viewingCard.background }}</div>
      </div>

      <div v-if="viewingCard.goals" class="detail-section">
        <div class="section-label">目标</div>
        <div class="section-content text-content">{{ viewingCard.goals }}</div>
      </div>

      <div v-if="viewingCard.relationships.length > 0" class="detail-section">
        <div class="section-label">人物关系</div>
        <div class="section-content">
          <div v-for="(rel, index) in viewingCard.relationships" :key="index" class="relationship-row">
            <span class="rel-target">{{ rel.target_name }}</span>
            <span class="rel-type">{{ rel.relationship }}</span>
            <span v-if="rel.description" class="rel-desc">{{ rel.description }}</span>
          </div>
        </div>
      </div>

      <div v-if="viewingCard.tags.length > 0" class="detail-section">
        <div class="section-label">标签</div>
        <div class="section-content">
          <el-tag v-for="(tag, index) in viewingCard.tags" :key="index" size="small" class="detail-tag">
            {{ tag }}
          </el-tag>
        </div>
      </div>

      <div v-if="viewingCard.notes" class="detail-section">
        <div class="section-label">备注</div>
        <div class="section-content text-content">{{ viewingCard.notes }}</div>
      </div>
    </div>
  </el-dialog>
</template>

<style scoped>
.character-drawer :deep(.el-drawer__body) {
  padding: 0;
}

.character-manager {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.toolbar {
  display: flex;
  gap: 12px;
  padding: 16px;
  border-bottom: 1px solid var(--el-border-color-light);
}

.search-input {
  flex: 1;
}

.character-list {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.character-card {
  padding: 16px;
  border: 1px solid var(--el-border-color-light);
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.25s ease;
  background: var(--el-bg-color);
}

.character-card:hover {
  border-color: var(--el-color-primary-light-5);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.06);
  transform: translateY(-2px);
}

.card-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 10px;
}

.card-avatar {
  width: 44px;
  height: 44px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 18px;
  font-weight: 600;
  flex-shrink: 0;
}

.avatar-text {
  line-height: 1;
}

.card-info {
  flex: 1;
  min-width: 0;
}

.card-name {
  font-weight: 600;
  font-size: 15px;
  color: var(--el-text-color-primary);
  margin-bottom: 4px;
}

.card-meta {
  display: flex;
  gap: 8px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  align-items: center;
}

.meta-item {
  background: var(--el-fill-color-light);
  padding: 2px 8px;
  border-radius: 4px;
  display: inline-flex;
  align-items: center;
  gap: 3px;
}

.card-actions {
  display: flex;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.2s;
}

.character-card:hover .card-actions {
  opacity: 1;
}

.card-aliases {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  margin-bottom: 8px;
}

.alias-tag {
  font-size: 12px;
  color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
  padding: 1px 8px;
  border-radius: 4px;
}

.more-aliases {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.card-tags {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  margin-bottom: 8px;
}

.card-tag {
  margin: 0;
}

.more-tags {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  padding: 2px 6px;
  line-height: 22px;
}

.card-preview {
  font-size: 13px;
  line-height: 1.5;
}

.preview-text {
  color: var(--el-text-color-regular);
}

.preview-placeholder {
  color: var(--el-text-color-placeholder);
  font-style: italic;
}

.empty-hint {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  margin-top: 4px;
}

/* 表单样式 */
.character-form {
  max-height: 60vh;
  overflow-y: auto;
  padding-right: 8px;
}

.dynamic-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.dynamic-item {
  display: flex;
  gap: 8px;
  align-items: center;
}

.relationship-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.relationship-item {
  display: flex;
  gap: 8px;
  align-items: center;
}

/* 详情样式 */
.character-detail {
  max-height: 60vh;
  overflow-y: auto;
  padding-right: 8px;
}

.detail-header {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 20px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--el-border-color-light);
}

.detail-avatar {
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

.detail-title h3 {
  margin: 0 0 8px 0;
  font-size: 18px;
}

.detail-meta {
  display: flex;
  gap: 8px;
}

.meta-tag {
  background: var(--el-fill-color-light);
  padding: 2px 10px;
  border-radius: 12px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.detail-section {
  margin-bottom: 16px;
}

.section-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 8px;
}

.section-content {
  color: var(--el-text-color-primary);
}

.text-content {
  line-height: 1.6;
  white-space: pre-wrap;
}

.detail-tag {
  margin-right: 6px;
  margin-bottom: 6px;
}

.relationship-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 0;
  border-bottom: 1px solid var(--el-border-color-light);
}

.relationship-row:last-child {
  border-bottom: none;
}

.rel-target {
  font-weight: 500;
  color: var(--el-color-primary);
}

.rel-type {
  background: var(--el-fill-color-light);
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 12px;
}

.rel-desc {
  color: var(--el-text-color-secondary);
  font-size: 13px;
}
</style>
