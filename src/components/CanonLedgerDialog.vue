<template>
  <el-dialog
    :model-value="visible"
    title="设定账本"
    width="900px"
    top="6vh"
    destroy-on-close
    @update:model-value="(value: boolean) => emit('update:modelValue', value)"
    @open="handleOpen"
  >
    <div class="canon-toolbar">
      <el-input
        v-model="keyword"
        placeholder="搜索名称 / 别名 / 简介"
        clearable
        size="small"
        style="width: 200px"
        @input="debouncedLoad"
      />
      <el-select v-model="kind" size="small" style="width: 110px" @change="loadEntities">
        <el-option label="全部类型" value="all" />
        <el-option label="角色" value="character" />
        <el-option label="地点" value="location" />
        <el-option label="势力" value="faction" />
        <el-option label="物品" value="item" />
        <el-option label="功法" value="skill" />
        <el-option label="术语" value="term" />
      </el-select>
      <el-button size="small" :icon="Refresh" @click="reload">刷新</el-button>
      <span class="canon-stats">
        实体 {{ stats.entities }} · 状态 {{ stats.states }} · 事件 {{ stats.events }} · 已抽取 {{ stats.extractedChapters }} 章
      </span>
      <el-button type="primary" size="small" :loading="rebuilding" @click="rebuild">
        {{ rebuilding ? '回填中…' : '一键回填' }}
      </el-button>
    </div>

    <el-progress
      v-if="rebuilding"
      :percentage="progressPercent"
      :stroke-width="12"
      :format="() => `${rebuildDone}/${rebuildTotal}`"
      style="margin: 8px 0"
    />

    <div class="canon-body">
      <div class="canon-list">
        <div
          v-for="entity in entities"
          :key="entity.id"
          class="canon-item"
          :class="{ active: selected && selected.id === entity.id }"
          @click="openEntity(entity.id)"
        >
          <span class="canon-kind">{{ kindLabel(entity.kind) }}</span>
          <span class="canon-name">{{ entity.name }}</span>
          <span class="canon-chapters">第{{ entity.firstChapter }}-{{ entity.lastChapter }}章</span>
          <span v-if="entity.manual" class="canon-manual">手改</span>
        </div>
        <div v-if="!entities.length" class="canon-empty">账本还是空的，点右上角「一键回填」</div>
      </div>

      <div class="canon-detail" v-if="selected">
        <div class="detail-header">
          <el-input v-model="form.name" size="small" style="width: 150px" placeholder="规范名" />
          <el-select v-model="form.kind" size="small" style="width: 100px">
            <el-option label="角色" value="character" />
            <el-option label="地点" value="location" />
            <el-option label="势力" value="faction" />
            <el-option label="物品" value="item" />
            <el-option label="功法" value="skill" />
            <el-option label="术语" value="term" />
          </el-select>
          <el-button size="small" type="primary" @click="saveEntity">保存</el-button>
          <el-button size="small" type="danger" plain @click="removeEntity">删除</el-button>
        </div>

        <el-input
          v-model="form.aliasesText"
          size="small"
          placeholder="别名，用顿号或逗号分隔（如：小动、林兄）"
          style="margin-top: 8px"
        />
        <el-input
          v-model="form.summary"
          type="textarea"
          :rows="3"
          size="small"
          placeholder="稳定特征（身份、能力、外貌、背景）"
          style="margin-top: 8px"
        />

        <div class="detail-section">
          <div class="section-title">状态时间线</div>
          <div v-for="state in selected.states" :key="state.id" class="timeline-row">
            <span class="chapter">第{{ state.chapterOrder }}章</span>
            <span class="timeline-text">{{ state.field }} = {{ state.value }}</span>
            <el-button link type="danger" size="small" @click="removeState(state.id)">删除</el-button>
          </div>
          <div v-if="!selected.states.length" class="empty-tip">暂无状态记录</div>
        </div>

        <div class="detail-section">
          <div class="section-title">事件</div>
          <div v-for="event in selected.events" :key="event.id" class="timeline-row">
            <span class="chapter">第{{ event.chapterOrder }}章</span>
            <span class="timeline-text">{{ event.predicate }}{{ event.object }}</span>
            <el-button link type="danger" size="small" @click="removeEvent(event.id)">删除</el-button>
          </div>
          <div v-if="!selected.events.length" class="empty-tip">暂无事件记录</div>
        </div>
      </div>
      <div v-else class="canon-detail canon-placeholder">选择左侧实体查看 / 修正设定</div>
    </div>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { ElMessage, ElMessageBox } from 'element-plus';
import { Refresh } from '@element-plus/icons-vue';

const props = defineProps<{ modelValue: boolean; bookId: string }>();
const emit = defineEmits<{ (e: 'update:modelValue', value: boolean): void }>();

const visible = computed({
  get: () => props.modelValue,
  set: (value: boolean) => emit('update:modelValue', value),
});

const keyword = ref('');
const kind = ref('all');
const entities = ref<any[]>([]);
const selected = ref<any | null>(null);
const stats = ref({ entities: 0, states: 0, events: 0, extractedChapters: 0 });

const rebuilding = ref(false);
const rebuildDone = ref(0);
const rebuildTotal = ref(0);
const progressPercent = computed(() =>
  rebuildTotal.value ? Math.round((rebuildDone.value / rebuildTotal.value) * 100) : 0
);

const form = reactive({ name: '', kind: 'character', summary: '', aliasesText: '' });

const kindLabel = (value: string) => {
  const map: Record<string, string> = {
    character: '角色',
    location: '地点',
    faction: '势力',
    item: '物品',
    skill: '功法',
    term: '术语',
  };
  return map[value] || '设定';
};

let debounceTimer: number | null = null;
const debouncedLoad = () => {
  if (debounceTimer) window.clearTimeout(debounceTimer);
  debounceTimer = window.setTimeout(() => loadEntities(), 250);
};

const loadStats = async () => {
  try {
    stats.value = await invoke<any>('canon_stats', { bookId: props.bookId });
  } catch (error) {
    console.error('加载账本统计失败:', error);
  }
};

const loadEntities = async () => {
  try {
    entities.value = await invoke<any[]>('list_canon_entities', {
      bookId: props.bookId,
      keyword: keyword.value || null,
      kind: kind.value === 'all' ? null : kind.value,
    });
  } catch (error) {
    ElMessage.error(`加载账本失败：${error}`);
  }
};

const reload = async () => {
  await Promise.all([loadEntities(), loadStats()]);
};

const openEntity = async (entityId: string) => {
  try {
    const detail = await invoke<any>('get_canon_entity', {
      bookId: props.bookId,
      entityId,
    });
    if (detail.error) {
      ElMessage.warning(detail.error);
      return;
    }
    selected.value = detail;
    form.name = detail.name;
    form.kind = detail.kind;
    form.summary = detail.summary;
    form.aliasesText = (detail.aliases || []).join('、');
  } catch (error) {
    ElMessage.error(`加载实体失败：${error}`);
  }
};

const parseAliases = () =>
  form.aliasesText
    .split(/[、,，;；\s]+/)
    .map((item) => item.trim())
    .filter(Boolean);

const saveEntity = async () => {
  if (!selected.value) return;
  try {
    await invoke('update_canon_entity', {
      bookId: props.bookId,
      entityId: selected.value.id,
      name: form.name,
      kind: form.kind,
      summary: form.summary,
      aliases: parseAliases(),
    });
    ElMessage.success('已保存');
    await reload();
    await openEntity(selected.value.id);
  } catch (error) {
    ElMessage.error(`保存失败：${error}`);
  }
};

const removeEntity = async () => {
  if (!selected.value) return;
  try {
    await ElMessageBox.confirm(
      `删除实体「${selected.value.name}」及其状态/事件？`,
      '确认删除',
      { type: 'warning' }
    );
  } catch {
    return;
  }
  try {
    await invoke('delete_canon_entity', {
      bookId: props.bookId,
      entityId: selected.value.id,
    });
    selected.value = null;
    ElMessage.success('已删除');
    await reload();
  } catch (error) {
    ElMessage.error(`删除失败：${error}`);
  }
};

const removeState = async (stateId: string) => {
  try {
    await invoke('delete_canon_state', { bookId: props.bookId, stateId });
    if (selected.value) await openEntity(selected.value.id);
    await loadStats();
  } catch (error) {
    ElMessage.error(`删除失败：${error}`);
  }
};

const removeEvent = async (eventId: string) => {
  try {
    await invoke('delete_canon_event', { bookId: props.bookId, eventId });
    if (selected.value) await openEntity(selected.value.id);
    await loadStats();
  } catch (error) {
    ElMessage.error(`删除失败：${error}`);
  }
};

const rebuild = async () => {
  try {
    await ElMessageBox.confirm(
      '将逐章重新抽取设定，会调用 AI（每章一次），耗时取决于章节数。继续？',
      '一键回填',
      { type: 'warning' }
    );
  } catch {
    return;
  }

  rebuilding.value = true;
  rebuildDone.value = 0;
  rebuildTotal.value = 0;

  try {
    await invoke('rebuild_canon', { bookId: props.bookId });
  } catch (error) {
    rebuilding.value = false;
    ElMessage.error(`回填启动失败：${error}`);
  }
};

const handleOpen = () => {
  reload();
};

let progressUnlisten: UnlistenFn | null = null;
let finishedUnlisten: UnlistenFn | null = null;

onMounted(async () => {
  progressUnlisten = await listen<any>('canon-rebuild-progress', (event) => {
    if (event.payload?.bookId !== props.bookId) return;
    rebuildDone.value = event.payload.done || 0;
    rebuildTotal.value = event.payload.total || 0;
  });
  finishedUnlisten = await listen<any>('canon-rebuild-finished', (event) => {
    if (event.payload?.bookId !== props.bookId) return;
    rebuilding.value = false;
    rebuildDone.value = 0;
    rebuildTotal.value = 0;
    ElMessage.success('回填完成');
    reload();
  });
});

onUnmounted(() => {
  if (debounceTimer) window.clearTimeout(debounceTimer);
  progressUnlisten?.();
  finishedUnlisten?.();
});
</script>

<style scoped>
.canon-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.canon-stats {
  font-size: 12px;
  color: var(--nw-text-secondary);
  margin-left: auto;
}

.canon-body {
  display: flex;
  gap: 12px;
  margin-top: 10px;
  height: 56vh;
}

.canon-list {
  width: 300px;
  overflow-y: auto;
  border: 1px solid var(--nw-border);
  border-radius: var(--nw-radius);
  padding: 4px;
}

.canon-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  border-radius: var(--nw-radius-sm);
  cursor: pointer;
  font-size: 13px;
}

.canon-item:hover {
  background: var(--nw-bg-secondary);
}

.canon-item.active {
  background: var(--el-color-primary-light-9);
}

.canon-kind {
  flex-shrink: 0;
  font-size: 11px;
  color: var(--nw-text-tertiary);
  border: 1px solid var(--nw-border);
  border-radius: 4px;
  padding: 0 4px;
}

.canon-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.canon-chapters {
  font-size: 11px;
  color: var(--nw-text-tertiary);
}

.canon-manual {
  font-size: 11px;
  color: var(--el-color-warning);
}

.canon-empty {
  padding: 24px 8px;
  text-align: center;
  font-size: 12px;
  color: var(--nw-text-tertiary);
}

.canon-detail {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  border: 1px solid var(--nw-border);
  border-radius: var(--nw-radius);
  padding: 10px;
}

.canon-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  color: var(--nw-text-tertiary);
}

.detail-header {
  display: flex;
  gap: 6px;
  align-items: center;
}

.detail-section {
  margin-top: 12px;
}

.section-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--nw-text-secondary);
  margin-bottom: 6px;
}

.timeline-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  padding: 3px 0;
  border-bottom: 1px dashed var(--nw-border);
}

.timeline-row .chapter {
  flex-shrink: 0;
  color: var(--nw-text-tertiary);
  width: 64px;
}

.timeline-text {
  flex: 1;
  min-width: 0;
  word-break: break-word;
}

.empty-tip {
  font-size: 12px;
  color: var(--nw-text-tertiary);
}
</style>
