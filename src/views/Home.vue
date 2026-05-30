<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useBookStore } from '../stores/book';
import { useConfigStore } from '../stores/config';
import { invoke } from '@tauri-apps/api/core';
import { Setting, Plus, Delete, Collection, Document, EditPen } from '@element-plus/icons-vue';
import { ElMessage, ElMessageBox } from 'element-plus';

const bookStore = useBookStore();
const configStore = useConfigStore();

const showCreateDialog = ref(false);
const newBookTitle = ref('');
const newBookAuthor = ref('');
const newBookDescription = ref('');

const showEditDialog = ref(false);
const editingBookId = ref('');
const editBookTitle = ref('');
const editBookAuthor = ref('');
const editBookDescription = ref('');

onMounted(async () => {
  await configStore.init();
  await bookStore.updateBooksList();
});

const createBook = async () => {
  if (!newBookTitle.value.trim()) {
    ElMessage.warning('请输入书名');
    return;
  }

  await bookStore.createBook(
    newBookTitle.value.trim(),
    newBookAuthor.value.trim(),
    newBookDescription.value.trim()
  );

  showCreateDialog.value = false;
  newBookTitle.value = '';
  newBookAuthor.value = '';
  newBookDescription.value = '';
  ElMessage.success('书籍创建成功');
};

const openBook = (bookId: string) => {
  bookStore.loadBook(bookId);
};

const handleDeleteBook = async (bookId: string) => {
  try {
    await ElMessageBox.confirm('确定要删除这本书吗？此操作不可恢复。', '确认删除', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    });
    await bookStore.deleteBook(bookId);
    ElMessage.success('书籍已删除');
  } catch {
    // 用户取消
  }
};

const openEditDialog = (book: any) => {
  editingBookId.value = book.id;
  editBookTitle.value = book.title;
  editBookAuthor.value = book.author || '';
  editBookDescription.value = book.description || '';
  showEditDialog.value = true;
};

const confirmEditBook = async () => {
  if (!editBookTitle.value.trim()) {
    ElMessage.warning('请输入书名');
    return;
  }

  try {
    await invoke('update_book', {
      bookId: editingBookId.value,
      title: editBookTitle.value.trim(),
      author: editBookAuthor.value.trim(),
      description: editBookDescription.value.trim(),
    });
    showEditDialog.value = false;
    ElMessage.success('书籍信息已更新');
    await bookStore.updateBooksList();
  } catch (error) {
    ElMessage.error('更新失败：' + error);
  }
};

const formatDate = (timestamp: number) => {
  return new Date(timestamp).toLocaleDateString('zh-CN');
};
</script>

<template>
  <div class="home">
    <!-- 顶部导航栏 -->
    <header class="home-header">
      <div class="header-brand">
        <div class="brand-icon">
          <el-icon :size="28"><EditPen /></el-icon>
        </div>
        <div class="brand-text">
          <h1>白喵写作</h1>
          <span class="brand-tagline">专注创作，记录灵感</span>
        </div>
      </div>
      <el-button
        class="settings-btn"
        :icon="Setting"
        text
        @click="configStore.openSettings"
      >
        设置
      </el-button>
    </header>

    <!-- 主内容区 -->
    <main class="home-main">
      <div class="content-wrapper">
        <!-- 页面标题栏 -->
        <div class="page-header">
          <div class="page-title">
            <el-icon :size="24"><Collection /></el-icon>
            <h2>我的作品</h2>
          </div>
          <el-button type="primary" :icon="Plus" @click="showCreateDialog = true">
            新建书籍
          </el-button>
        </div>

        <!-- 书籍网格 -->
        <div v-if="bookStore.booksList.length > 0" class="books-grid">
          <div
            v-for="book in bookStore.booksList"
            :key="book.id"
            class="book-card"
            @click="openBook(book.id)"
          >
            <div class="book-cover">
              <div class="cover-decoration">
                <div class="cover-spine"></div>
                <div class="cover-pattern"></div>
              </div>
              <div class="cover-icon">
                <el-icon :size="32"><Document /></el-icon>
              </div>
            </div>
            <div class="book-details">
              <h3 class="book-title">{{ book.title }}</h3>
              <p class="book-meta">
                <span class="update-date">{{ formatDate(book.updatedAt) }} 更新</span>
              </p>
            </div>
            <div class="book-actions">
              <el-button
                class="edit-btn"
                circle
                size="small"
                :icon="EditPen"
                @click.stop="openEditDialog(book)"
              />
              <el-button
                class="delete-btn"
                circle
                size="small"
                type="danger"
                :icon="Delete"
                @click.stop="handleDeleteBook(book.id)"
              />
            </div>
          </div>

          <!-- 创建新书卡片 -->
          <div class="book-card create-card" @click="showCreateDialog = true">
            <div class="create-content">
              <div class="create-icon">
                <el-icon :size="36"><Plus /></el-icon>
              </div>
              <div class="create-text">
                <h3>创建新书</h3>
                <p>开启一段新的创作旅程</p>
              </div>
            </div>
          </div>
        </div>

        <!-- 空状态 -->
        <div v-else class="empty-state">
          <div class="empty-illustration">
            <div class="empty-book">
              <el-icon :size="48"><Document /></el-icon>
            </div>
          </div>
          <h3>还没有书籍</h3>
          <p>创建你的第一本书，开始记录你的故事</p>
          <el-button type="primary" :icon="Plus" @click="showCreateDialog = true">
            创建新书
          </el-button>
        </div>
      </div>
    </main>

    <!-- 创建书籍对话框 -->
    <el-dialog
      v-model="showCreateDialog"
      title="创建新书"
      width="520px"
      destroy-on-close
      class="create-book-dialog"
    >
      <el-form label-position="top" class="create-form">
        <el-form-item label="书名">
          <el-input
            v-model="newBookTitle"
            placeholder="给你的作品起个名字"
            @keyup.enter="createBook"
            autofocus
            size="large"
          />
        </el-form-item>
        <el-form-item label="作者">
          <el-input v-model="newBookAuthor" placeholder="你的名字" size="large" />
        </el-form-item>
        <el-form-item label="简介">
          <el-input
            v-model="newBookDescription"
            type="textarea"
            :rows="4"
            placeholder="简单描述一下这本书的内容..."
            resize="none"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showCreateDialog = false" size="large">取消</el-button>
        <el-button 
          type="primary" 
          @click="createBook" 
          :disabled="!newBookTitle.trim()"
          size="large"
        >
          创建
        </el-button>
      </template>
    </el-dialog>

    <!-- 编辑书籍对话框 -->
    <el-dialog
      v-model="showEditDialog"
      title="编辑书籍信息"
      width="520px"
      destroy-on-close
      class="create-book-dialog"
    >
      <el-form label-position="top" class="create-form">
        <el-form-item label="书名">
          <el-input
            v-model="editBookTitle"
            placeholder="给你的作品起个名字"
            @keyup.enter="confirmEditBook"
            autofocus
            size="large"
          />
        </el-form-item>
        <el-form-item label="作者">
          <el-input v-model="editBookAuthor" placeholder="你的名字" size="large" />
        </el-form-item>
        <el-form-item label="简介">
          <el-input
            v-model="editBookDescription"
            type="textarea"
            :rows="4"
            placeholder="简单描述一下这本书的内容..."
            resize="none"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showEditDialog = false" size="large">取消</el-button>
        <el-button 
          type="primary" 
          @click="confirmEditBook" 
          :disabled="!editBookTitle.trim()"
          size="large"
        >
          保存
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.home {
  min-height: 100vh;
  background: var(--nw-bg-page);
  display: flex;
  flex-direction: column;
}

/* 顶部导航栏 */
.home-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 var(--nw-space-2xl);
  height: 72px;
  background: var(--nw-bg-primary);
  border-bottom: 1px solid var(--nw-border-light);
  flex-shrink: 0;
}

.header-brand {
  display: flex;
  align-items: center;
  gap: var(--nw-space-md);
}

.brand-icon {
  width: 44px;
  height: 44px;
  background: linear-gradient(135deg, var(--nw-primary) 0%, var(--nw-primary-light) 100%);
  border-radius: var(--nw-radius);
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  box-shadow: var(--nw-shadow);
}

.brand-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.brand-text h1 {
  font-size: 20px;
  font-weight: 700;
  color: var(--nw-text-primary);
  margin: 0;
  font-family: var(--nw-font-display);
  letter-spacing: 1px;
}

.brand-tagline {
  font-size: 12px;
  color: var(--nw-text-tertiary);
  font-weight: 400;
}

.settings-btn {
  font-size: 14px;
  color: var(--nw-text-secondary);
  padding: 8px 16px;
}

/* 主内容区 */
.home-main {
  flex: 1;
  padding: var(--nw-space-2xl);
  overflow-y: auto;
}

.content-wrapper {
  max-width: 1200px;
  margin: 0 auto;
}

/* 页面标题栏 */
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--nw-space-xl);
}

.page-title {
  display: flex;
  align-items: center;
  gap: var(--nw-space-sm);
  color: var(--nw-text-primary);
}

.page-title h2 {
  font-size: 24px;
  font-weight: 600;
  margin: 0;
  font-family: var(--nw-font-display);
}

.page-title .el-icon {
  color: var(--nw-primary);
}

/* 书籍网格 */
.books-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: var(--nw-space-lg);
}

/* 书籍卡片 */
.book-card {
  background: var(--nw-bg-primary);
  border: 1px solid var(--nw-border-light);
  border-radius: var(--nw-radius-md);
  padding: var(--nw-space-lg);
  cursor: pointer;
  transition: all var(--nw-transition);
  position: relative;
  display: flex;
  gap: var(--nw-space-md);
  align-items: flex-start;
}

.book-card:hover {
  border-color: var(--nw-border);
  box-shadow: var(--nw-shadow-md);
  transform: translateY(-2px);
}

.book-cover {
  width: 72px;
  height: 96px;
  background: linear-gradient(145deg, var(--nw-primary) 0%, var(--nw-primary-dark) 100%);
  border-radius: var(--nw-radius-sm);
  position: relative;
  flex-shrink: 0;
  overflow: hidden;
  box-shadow: var(--nw-shadow);
}

.cover-decoration {
  position: absolute;
  inset: 0;
}

.cover-spine {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 6px;
  background: linear-gradient(to right, rgba(255,255,255,0.2), rgba(255,255,255,0.05));
}

.cover-pattern {
  position: absolute;
  right: 8px;
  top: 8px;
  width: 24px;
  height: 24px;
  border: 2px solid rgba(255,255,255,0.15);
  border-radius: 50%;
}

.cover-icon {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: rgba(255,255,255,0.9);
}

.book-details {
  flex: 1;
  min-width: 0;
  padding-top: var(--nw-space-xs);
}

.book-title {
  font-size: 17px;
  font-weight: 600;
  color: var(--nw-text-primary);
  margin: 0 0 var(--nw-space-sm) 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--nw-font-display);
}

.book-meta {
  margin: 0;
}

.update-date {
  font-size: 12px;
  color: var(--nw-text-tertiary);
  background: var(--nw-bg-secondary);
  padding: 4px 10px;
  border-radius: 12px;
}

.book-actions {
  position: absolute;
  top: var(--nw-space);
  right: var(--nw-space);
  display: flex;
  gap: var(--nw-space-xs);
}

.edit-btn,
.delete-btn {
  opacity: 0;
  transition: opacity var(--nw-transition-fast);
}

.book-card:hover .edit-btn,
.book-card:hover .delete-btn {
  opacity: 1;
}

/* 创建新书卡片 */
.create-card {
  border: 2px dashed var(--nw-border);
  background: transparent;
}

.create-card:hover {
  border-color: var(--nw-primary);
  background: var(--nw-bg-secondary);
}

.create-content {
  display: flex;
  align-items: center;
  gap: var(--nw-space-md);
  width: 100%;
}

.create-icon {
  width: 72px;
  height: 96px;
  border: 2px dashed var(--nw-border);
  border-radius: var(--nw-radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--nw-text-tertiary);
  flex-shrink: 0;
}

.create-card:hover .create-icon {
  border-color: var(--nw-primary);
  color: var(--nw-primary);
}

.create-text h3 {
  font-size: 16px;
  font-weight: 600;
  color: var(--nw-text-primary);
  margin: 0 0 4px 0;
}

.create-text p {
  font-size: 13px;
  color: var(--nw-text-tertiary);
  margin: 0;
}

/* 空状态 */
.empty-state {
  text-align: center;
  padding: var(--nw-space-2xl) 0;
}

.empty-illustration {
  margin-bottom: var(--nw-space-lg);
}

.empty-book {
  width: 120px;
  height: 150px;
  background: linear-gradient(145deg, var(--nw-bg-secondary) 0%, var(--nw-bg-tertiary) 100%);
  border-radius: var(--nw-radius);
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0 auto;
  color: var(--nw-text-muted);
  border: 2px dashed var(--nw-border);
}

.empty-state h3 {
  font-size: 20px;
  font-weight: 600;
  color: var(--nw-text-primary);
  margin: 0 0 var(--nw-space-sm) 0;
  font-family: var(--nw-font-display);
}

.empty-state p {
  font-size: 14px;
  color: var(--nw-text-secondary);
  margin: 0 0 var(--nw-space-lg) 0;
}

/* 创建对话框 */
.create-book-dialog :deep(.el-dialog__header) {
  padding: var(--nw-space-lg) var(--nw-space-xl);
}

.create-book-dialog :deep(.el-dialog__body) {
  padding: var(--nw-space-lg) var(--nw-space-xl);
}

.create-book-dialog :deep(.el-dialog__footer) {
  padding: var(--nw-space) var(--nw-space-xl) var(--nw-space-lg);
}

.create-form :deep(.el-form-item__label) {
  font-size: 14px;
  font-weight: 500;
  color: var(--nw-text-secondary);
  padding-bottom: 6px;
}

.create-form :deep(.el-input__inner) {
  font-size: 15px;
}

.create-form :deep(.el-textarea__inner) {
  font-size: 15px;
  line-height: 1.6;
}
</style>
