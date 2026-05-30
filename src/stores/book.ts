import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Book, Chapter, Volume, ChatMessage, SnapshotComparison, CharacterCard, Outline } from '../types';

// 后端返回的 Volume 数据
interface VolumeData {
  id: string;
  title: string;
  order: number;
  is_collapsed: boolean;
  created_at: number;
  updated_at: number;
}

// 后端返回的书籍元数据（不含章节内容）
interface BookMeta {
  id: string;
  title: string;
  author: string;
  description: string;
  volumes: VolumeData[];
  chapters: ChapterMeta[];
  current_chapter_id: string | null;
  created_at: number;
  updated_at: number;
}

interface ChapterMeta {
  id: string;
  title: string;
  order: number;
  volume_id: string;
  content: string;
  word_count: number;
  created_at: number;
  updated_at: number;
}

export const useBookStore = defineStore('book', () => {
  // State
  const currentBook = ref<Book | null>(null);
  const currentChapterId = ref<string | null>(null);
  const chatMessages = ref<ChatMessage[]>([]);
  const isSaving = ref(false);
  const lastSavedAt = ref<number>(0);
  const booksList = ref<{ id: string; title: string; author: string; description: string; updatedAt: number }[]>([]);
  // 缓存章节内容（避免频繁读写文件）
  const chapterContents = ref<Map<string, string>>(new Map());
  
  // 自动快照相关
  const snapshotDebounceTimer = ref<number | null>(null);
  const hasUnsavedChanges = ref(false);

  // Getters
  const volumes = computed(() => {
    if (!currentBook.value) return [];
    return [...currentBook.value.volumes].sort((a, b) => a.order - b.order);
  });

  const chapters = computed(() => {
    if (!currentBook.value) return [];
    return [...currentBook.value.chapters].sort((a, b) => a.order - b.order);
  });

  const getChaptersByVolume = (volumeId: string) => {
    return chapters.value.filter(c => c.volumeId === volumeId);
  };

  const currentChapter = computed(() => {
    if (!currentBook.value || !currentChapterId.value) return null;
    return currentBook.value.chapters.find(c => c.id === currentChapterId.value) || null;
  });

  // 当前章节内容（从缓存或加载）
  const currentContent = computed({
    get: () => {
      if (!currentChapter.value) return '';
      return chapterContents.value.get(currentChapter.value.id) || '';
    },
    set: (value: string) => {
      if (currentChapter.value) {
        chapterContents.value.set(currentChapter.value.id, value);
        currentChapter.value.updatedAt = Date.now();
        currentBook.value!.updatedAt = Date.now();
      }
    }
  });

  const wordCount = computed(() => {
    return currentContent.value.length;
  });

  const totalWordCount = computed(() => {
    if (!currentBook.value) return 0;
    // 这里只计算当前缓存的内容，实际字数需要遍历所有TXT文件
    return currentBook.value.chapters.reduce((sum, c) => {
      const content = chapterContents.value.get(c.id) || '';
      return sum + content.length;
    }, 0);
  });

  // 将后端 BookMeta 转换为前端 Book 格式
  const convertBookMeta = (meta: BookMeta): Book => {
    return {
      id: meta.id,
      title: meta.title,
      author: meta.author,
      description: meta.description,
      volumes: meta.volumes.map(v => ({
        id: v.id,
        title: v.title,
        order: v.order,
        isCollapsed: v.is_collapsed ?? false,
        createdAt: v.created_at ?? Date.now(),
        updatedAt: v.updated_at ?? Date.now()
      })),
      chapters: meta.chapters.map(c => ({
        id: c.id,
        title: c.title,
        content: c.content || '', // 内容从数据库加载
        order: c.order,
        volumeId: c.volume_id,
        createdAt: c.created_at,
        updatedAt: c.updated_at
      })),
      currentChapterId: meta.current_chapter_id,
      createdAt: meta.created_at,
      updatedAt: meta.updated_at
    };
  };

  // 将前端 Book 转换为后端 BookMeta 格式
  const convertToBookMeta = (book: Book): BookMeta => {
    return {
      id: book.id,
      title: book.title,
      author: book.author,
      description: book.description,
      volumes: book.volumes.map(v => ({
        id: v.id,
        title: v.title,
        order: v.order,
        is_collapsed: v.isCollapsed,
        created_at: v.createdAt,
        updated_at: v.updatedAt
      })),
      chapters: book.chapters.map(c => ({
        id: c.id,
        title: c.title,
        order: c.order,
        volume_id: c.volumeId,
        content: chapterContents.value.get(c.id) || '',
        word_count: (chapterContents.value.get(c.id) || '').length,
        created_at: c.createdAt,
        updated_at: c.updatedAt
      })),
      current_chapter_id: book.currentChapterId,
      created_at: book.createdAt,
      updated_at: book.updatedAt
    };
  };

  // Actions
  const createBook = async (title: string, author: string = '', description: string = '') => {
    try {
      const meta = await invoke<BookMeta>('create_book', { title, author, description });
      const book = convertBookMeta(meta);
      currentBook.value = book;
      currentChapterId.value = null;
      chapterContents.value.clear();
      
      // 创建默认第一章（必须在有卷的情况下）
      if (book.volumes.length > 0) {
        const chapter = await createChapter('第一章', book.volumes[0].id);
        if (!chapter) {
          throw new Error('创建默认章节失败');
        }
      } else {
        throw new Error('书籍没有卷，无法创建章节');
      }
      
      await updateBooksList();
      return book;
    } catch (error) {
      console.error('创建书籍失败:', error);
      throw error;
    }
  };

  const loadBook = async (bookId: string) => {
    try {
      // 加载书籍元数据（包含所有章节内容）
      const meta = await invoke<BookMeta>('load_book', { bookId });
      const book = convertBookMeta(meta);
      currentBook.value = book;
      currentChapterId.value = book.currentChapterId;
      chapterContents.value.clear();
      lastSavedAt.value = 0; // 重置保存时间
      
      // 将所有章节内容存入缓存
      book.chapters.forEach(chapter => {
        chapterContents.value.set(chapter.id, chapter.content);
      });
      
      await updateBooksList();
    } catch (error) {
      console.error('加载书籍失败:', error);
      throw error;
    }
  };

  // 防抖用的变量
  let saveTimeout: number | null = null;
  let pendingSaveResolve: (() => void) | null = null;

  const saveBook = async (): Promise<void> => {
    if (!currentBook.value) return;

    // 如果正在保存，等待当前保存完成
    if (isSaving.value) {
      return new Promise((resolve) => {
        const checkInterval = setInterval(() => {
          if (!isSaving.value) {
            clearInterval(checkInterval);
            resolve();
          }
        }, 100);
      });
    }

    // 清除之前的防抖定时器
    if (saveTimeout) {
      clearTimeout(saveTimeout);
      saveTimeout = null;
    }

    // 创建新的防抖保存
    return new Promise((resolve, reject) => {
      pendingSaveResolve = resolve;

      saveTimeout = window.setTimeout(async () => {
        isSaving.value = true;
        const startTime = Date.now();

        try {
          // 保存当前章节内容
          if (currentChapter.value && currentChapterId.value) {
            await invoke('save_chapter_content', {
              bookId: currentBook.value!.id,
              chapterId: currentChapterId.value,
              content: currentContent.value
            });
          }

          // 保存书籍元数据
          const book = convertToBookMeta(currentBook.value!);
          await invoke('save_book', { book });

          lastSavedAt.value = Date.now();
          pendingSaveResolve?.();
        } catch (error) {
          console.error('保存失败:', error);
          reject(error);
        } finally {
          // 确保"保存中"状态至少显示 500ms，让用户能看到
          const elapsed = Date.now() - startTime;
          const minDisplayTime = 500;
          if (elapsed < minDisplayTime) {
            await new Promise(resolve => setTimeout(resolve, minDisplayTime - elapsed));
          }
          isSaving.value = false;
          saveTimeout = null;
          pendingSaveResolve = null;
        }
      }, 300); // 300ms 防抖延迟
    });
  };

  const loadChapterContent = async (bookId: string, chapterId: string) => {
    try {
      const content = await invoke<string>('load_chapter_content', { bookId, chapterId });
      chapterContents.value.set(chapterId, content);
    } catch (error) {
      console.error('加载章节内容失败:', error);
      chapterContents.value.set(chapterId, '');
    }
  };

  const selectChapter = async (chapterId: string) => {
    if (!currentBook.value) return;
    
    // 如果切换章节，先保存当前章节
    if (currentChapterId.value && currentChapterId.value !== chapterId) {
      await saveBook();
    }
    
    currentChapterId.value = chapterId;
    currentBook.value.currentChapterId = chapterId;
    
    // 加载新章节内容
    if (!chapterContents.value.has(chapterId)) {
      await loadChapterContent(currentBook.value.id, chapterId);
    }
    
    // 保存书籍状态
    await saveBook();
  };

  const createChapter = async (title: string, volumeId: string) => {
    if (!currentBook.value) return null;
    
    try {
      const chapterMeta = await invoke<ChapterMeta>('create_chapter', {
        bookId: currentBook.value.id,
        title,
        volumeId
      });
      
      const chapter: Chapter = {
        id: chapterMeta.id,
        title: chapterMeta.title,
        content: '',
        order: chapterMeta.order,
        volumeId: chapterMeta.volume_id,
        createdAt: chapterMeta.created_at,
        updatedAt: chapterMeta.updated_at
      };
      
      currentBook.value.chapters.push(chapter);
      chapterContents.value.set(chapter.id, '');
      
      // 自动创建对应的大纲记录（粗纲+细纲）
      try {
        await Promise.all([
          saveOutline({
            book_id: currentBook.value.id,
            chapter_id: chapter.id,
            outline_type: 'coarse',
            content: ''
          }),
          saveOutline({
            book_id: currentBook.value.id,
            chapter_id: chapter.id,
            outline_type: 'fine',
            content: ''
          })
        ]);
      } catch (e) {
        console.warn('创建章节大纲记录失败:', e);
      }
      
      // 自动选中新章节
      await selectChapter(chapter.id);
      
      return chapter;
    } catch (error) {
      console.error('创建章节失败:', error);
      throw error;
    }
  };

  const deleteChapter = async (chapterId: string) => {
    if (!currentBook.value) return;
    
    try {
      await invoke('delete_chapter', {
        bookId: currentBook.value.id,
        chapterId
      });
      
      // 从本地状态中移除
      const index = currentBook.value.chapters.findIndex(c => c.id === chapterId);
      if (index > -1) {
        currentBook.value.chapters.splice(index, 1);
      }
      chapterContents.value.delete(chapterId);
      
      // 如果删除的是当前章节，清空选择
      if (currentChapterId.value === chapterId) {
        currentChapterId.value = null;
        if (currentBook.value) {
          currentBook.value.currentChapterId = null;
        }
      }
      
      await saveBook();
    } catch (error) {
      console.error('删除章节失败:', error);
      throw error;
    }
  };

  const updateChapterTitle = async (chapterId: string, title: string) => {
    if (!currentBook.value) return;
    
    const chapter = currentBook.value.chapters.find(c => c.id === chapterId);
    if (chapter) {
      chapter.title = title;
      chapter.updatedAt = Date.now();
      // 调用专门的更新章节标题命令
      await invoke('update_chapter_title', {
        bookId: currentBook.value.id,
        chapterId,
        title
      });
    }
  };

  const createVolume = async (title: string, order: number) => {
    if (!currentBook.value) return null;
    
    try {
      const volumeData = await invoke<VolumeData>('create_volume', {
        bookId: currentBook.value.id,
        title,
        order
      });
      
      const volume: Volume = {
        id: volumeData.id,
        title: volumeData.title,
        order: volumeData.order,
        isCollapsed: volumeData.is_collapsed ?? false,
        createdAt: volumeData.created_at ?? Date.now(),
        updatedAt: volumeData.updated_at ?? Date.now()
      };
      
      currentBook.value.volumes.push(volume);
      await saveBook();
      
      return volume;
    } catch (error) {
      console.error('创建卷失败:', error);
      throw error;
    }
  };

  const deleteVolume = async (volumeId: string) => {
    if (!currentBook.value) return;
    
    try {
      await invoke('delete_volume', {
        bookId: currentBook.value.id,
        volumeId
      });
      
      // 删除该卷下的所有章节
      const chaptersToDelete = currentBook.value.chapters.filter(c => c.volumeId === volumeId);
      for (const chapter of chaptersToDelete) {
        chapterContents.value.delete(chapter.id);
      }
      
      currentBook.value.chapters = currentBook.value.chapters.filter(c => c.volumeId !== volumeId);
      
      // 从卷列表中移除
      const index = currentBook.value.volumes.findIndex(v => v.id === volumeId);
      if (index > -1) {
        currentBook.value.volumes.splice(index, 1);
      }
      
      // 如果当前章节被删除，清空选择
      if (currentChapterId.value && chaptersToDelete.some(c => c.id === currentChapterId.value)) {
        currentChapterId.value = null;
        if (currentBook.value) {
          currentBook.value.currentChapterId = null;
        }
      }
      
      await saveBook();
    } catch (error) {
      console.error('删除卷失败:', error);
      throw error;
    }
  };

  const updateVolumeTitle = async (volumeId: string, title: string) => {
    if (!currentBook.value) return;
    
    try {
      await invoke('update_volume_title', { volumeId, title });
      
      const volume = currentBook.value.volumes.find(v => v.id === volumeId);
      if (volume) {
        volume.title = title;
        volume.updatedAt = Date.now();
      }
    } catch (error) {
      console.error('更新卷标题失败:', error);
      throw error;
    }
  };

  const toggleVolumeCollapse = (volumeId: string) => {
    if (!currentBook.value) return;
    
    const volume = currentBook.value.volumes.find(v => v.id === volumeId);
    if (volume) {
      volume.isCollapsed = !volume.isCollapsed;
    }
  };

  const updateBooksList = async () => {
    try {
      const list = await invoke<{ id: string; title: string; author: string; description: string; updated_at: number }[]>('get_books_list');
      booksList.value = list.map(b => ({
        id: b.id,
        title: b.title,
        author: b.author,
        description: b.description,
        updatedAt: b.updated_at
      }));
    } catch (error) {
      console.error('获取书籍列表失败:', error);
    }
  };

  const deleteBook = async (bookId: string) => {
    try {
      await invoke('delete_book', { bookId });
      
      // 如果删除的是当前打开的书，清空状态
      if (currentBook.value?.id === bookId) {
        currentBook.value = null;
        currentChapterId.value = null;
        chapterContents.value.clear();
      }
      
      await updateBooksList();
    } catch (error) {
      console.error('删除书籍失败:', error);
      throw error;
    }
  };

  // 自动保存
  let autoSaveInterval: number | null = null;

  const startAutoSave = (intervalMs: number = 30000) => {
    stopAutoSave();
    autoSaveInterval = window.setInterval(() => {
      if (currentBook.value && currentChapterId.value) {
        saveBook();
      }
    }, intervalMs);
  };

  const stopAutoSave = () => {
    if (autoSaveInterval) {
      clearInterval(autoSaveInterval);
      autoSaveInterval = null;
    }
  };

  // 聊天消息
  const addMessage = (message: ChatMessage) => {
    chatMessages.value.push(message);
  };

  const clearMessages = () => {
    chatMessages.value = [];
  };

  // ============ 章节快照功能 ============
  
  interface ChapterSnapshot {
    id: string;
    book_id: string;
    chapter_id: string;
    chapter_title: string;
    name: string;
    content: string;
    word_count: number;
    created_at: string;
  }

  // 当前章节的快照列表
  const currentChapterSnapshots = ref<ChapterSnapshot[]>([]);
  
  // 当前章节上次快照的内容
  const lastChapterSnapshotContent = ref<string>('');

  // 创建章节快照
  const createChapterSnapshot = async (name: string, chapterId?: string): Promise<ChapterSnapshot | null> => {
    if (!currentBook.value) return null;
    
    const targetChapterId = chapterId || currentChapterId.value;
    if (!targetChapterId) return null;
    
    const chapter = currentBook.value.chapters.find(c => c.id === targetChapterId);
    if (!chapter) return null;
    
    const content = chapterContents.value.get(targetChapterId) || '';
    
    try {
      const snapshot = await invoke<ChapterSnapshot>('create_chapter_snapshot', {
        bookId: currentBook.value.id,
        chapterId: targetChapterId,
        chapterTitle: chapter.title,
        name,
        content
      });
      
      // 清理旧快照，只保留最近20个
      await cleanupOldSnapshots(targetChapterId, 20);
      
      return snapshot;
    } catch (error) {
      console.error('创建章节快照失败:', error);
      throw error;
    }
  };

  // 获取章节快照列表
  const listChapterSnapshots = async (chapterId?: string): Promise<ChapterSnapshot[]> => {
    if (!currentBook.value) return [];
    
    const targetChapterId = chapterId || currentChapterId.value;
    if (!targetChapterId) return [];
    
    try {
      const snapshots = await invoke<ChapterSnapshot[]>('list_chapter_snapshots', {
        bookId: currentBook.value.id,
        chapterId: targetChapterId
      });
      
      currentChapterSnapshots.value = snapshots;
      return snapshots;
    } catch (error) {
      console.error('获取章节快照列表失败:', error);
      return [];
    }
  };

  // 恢复章节快照
  const restoreChapterSnapshot = async (snapshotId: string): Promise<boolean> => {
    if (!currentBook.value) return false;
    
    try {
      const snapshot = await invoke<ChapterSnapshot>('get_chapter_snapshot', {
        bookId: currentBook.value.id,
        snapshotId
      });
      
      // 恢复章节内容
      chapterContents.value.set(snapshot.chapter_id, snapshot.content);
      
      // 更新章节标题
      const chapter = currentBook.value.chapters.find(c => c.id === snapshot.chapter_id);
      if (chapter) {
        chapter.title = snapshot.chapter_title;
      }
      
      // 如果是当前章节，更新显示
      if (currentChapterId.value === snapshot.chapter_id) {
        lastChapterSnapshotContent.value = snapshot.content;
      }
      
      return true;
    } catch (error) {
      console.error('恢复章节快照失败:', error);
      throw error;
    }
  };

  // 删除章节快照
  const deleteChapterSnapshot = async (snapshotId: string): Promise<void> => {
    if (!currentBook.value) return;
    
    try {
      await invoke('delete_chapter_snapshot', {
        bookId: currentBook.value.id,
        snapshotId
      });
      
      // 从本地列表中移除
      currentChapterSnapshots.value = currentChapterSnapshots.value.filter(s => s.id !== snapshotId);
    } catch (error) {
      console.error('删除章节快照失败:', error);
      throw error;
    }
  };

  // 对比两个快照
  const compareSnapshots = async (oldSnapshotId: string, newSnapshotId: string): Promise<SnapshotComparison | null> => {
    if (!currentBook.value) return null;
    
    try {
      const result = await invoke<SnapshotComparison>('compare_snapshots', {
        bookId: currentBook.value.id,
        oldSnapshotId,
        newSnapshotId
      });
      return result;
    } catch (error) {
      console.error('对比快照失败:', error);
      throw error;
    }
  };

  // 清理旧快照
  const cleanupOldSnapshots = async (chapterId: string, keepCount: number): Promise<void> => {
    if (!currentBook.value) return;
    
    try {
      await invoke('cleanup_chapter_snapshots', {
        bookId: currentBook.value.id,
        chapterId,
        keepCount
      });
    } catch (error) {
      console.error('清理旧快照失败:', error);
    }
  };

  // ============ 自动快照功能 ============
  
  // 标记有未保存的修改
  const markAsModified = () => {
    hasUnsavedChanges.value = true;
  };
  
  // 清除修改标记
  const clearModified = (chapterId?: string) => {
    const targetId = chapterId || currentChapterId.value;
    if (targetId) {
      lastChapterSnapshotContent.value = chapterContents.value.get(targetId) || '';
    }
    hasUnsavedChanges.value = false;
  };
  
  // 检查当前章节内容是否有变化
  const hasChapterContentChanged = (chapterId?: string): boolean => {
    if (!hasUnsavedChanges.value) return false;
    
    const targetId = chapterId || currentChapterId.value;
    if (!targetId) return false;
    
    const currentContent = chapterContents.value.get(targetId) || '';
    return currentContent !== lastChapterSnapshotContent.value;
  };
  
  // 创建自动快照（带防抖）
  const createAutoSnapshot = (triggerType: string) => {
    if (!currentBook.value || !currentChapterId.value) return;
    
    // 清除之前的定时器
    if (snapshotDebounceTimer.value) {
      clearTimeout(snapshotDebounceTimer.value);
    }
    
    // 检查内容是否真的变化了
    if (!hasChapterContentChanged()) {
      return;
    }
    
    const chapterId = currentChapterId.value;
    const chapter = currentBook.value.chapters.find(c => c.id === chapterId);
    if (!chapter) return;
    
    // 防抖：3秒后执行
    snapshotDebounceTimer.value = window.setTimeout(async () => {
      try {
        // 再次检查，确保执行时仍有变化
        if (!hasChapterContentChanged(chapterId)) {
          return;
        }
        
        const timestamp = new Date().toLocaleString('zh-CN', {
          month: 'short',
          day: 'numeric',
          hour: '2-digit',
          minute: '2-digit'
        });
        
        const name = `自动保存 - ${triggerType} (${timestamp})`;

        await createChapterSnapshot(name, chapterId);
        clearModified(chapterId);
      } catch (error) {
        // 创建快照失败
      }
    }, 3000);
  };
  
  // 立即创建快照（用于关闭应用等场景）
    const createImmediateSnapshot = async (triggerType: string, chapterId?: string): Promise<void> => {
      if (!currentBook.value) return;
      
      const targetChapterId = chapterId || currentChapterId.value;
      if (!targetChapterId) return;
      
      // 清除防抖定时器
      if (snapshotDebounceTimer.value) {
        clearTimeout(snapshotDebounceTimer.value);
        snapshotDebounceTimer.value = null;
      }
      
      // 检查内容是否变化
      if (!hasChapterContentChanged(targetChapterId)) {
        return;
      }

      const chapter = currentBook.value.chapters.find(c => c.id === targetChapterId);
      if (!chapter) return;

      try {
        const timestamp = new Date().toLocaleString('zh-CN', {
          month: 'short',
          day: 'numeric',
          hour: '2-digit',
          minute: '2-digit'
        });

        const name = `自动保存 - ${triggerType} (${timestamp})`;

        await createChapterSnapshot(name, targetChapterId);
        clearModified(targetChapterId);
      } catch (error) {
        // 创建快照失败
      }
    };

    // ============ 角色卡功能 ============
    
    // 获取角色卡列表
    const listCharacterCards = async (): Promise<CharacterCard[]> => {
      if (!currentBook.value) return [];
      
      try {
        const cards = await invoke<CharacterCard[]>('list_character_cards', {
          bookId: currentBook.value.id
        });
        return cards.map(parseCardFields);
      } catch (error) {
        console.error('获取角色卡列表失败:', error);
        return [];
      }
    };
    
    // 创建角色卡
    const parseCardFields = (card: CharacterCard): CharacterCard => ({
      ...card,
      tags: typeof card.tags === 'string' ? JSON.parse(card.tags as string) : card.tags,
      aliases: typeof card.aliases === 'string' ? JSON.parse(card.aliases as string) : card.aliases,
      relationships: typeof card.relationships === 'string' ? JSON.parse(card.relationships as string) : card.relationships,
    });

    const createCharacterCard = async (card: CharacterCard): Promise<CharacterCard> => {
      if (!currentBook.value) throw new Error('没有打开的书籍');
      
      const result = await invoke<CharacterCard>('create_character_card', {
        bookId: currentBook.value.id,
        name: card.name,
        aliases: card.aliases,
        gender: card.gender,
        age: card.age,
        appearance: card.appearance,
        personality: card.personality,
        background: card.background,
        goals: card.goals,
        relationships: card.relationships,
        tags: card.tags,
        notes: card.notes
      });
      
      return parseCardFields(result);
    };
    
    // 更新角色卡
    const updateCharacterCard = async (card: CharacterCard): Promise<CharacterCard> => {
      const result = await invoke<CharacterCard>('update_character_card', {
        cardId: card.id,
        name: card.name,
        aliases: card.aliases,
        gender: card.gender,
        age: card.age,
        appearance: card.appearance,
        personality: card.personality,
        background: card.background,
        goals: card.goals,
        relationships: card.relationships,
        tags: card.tags,
        notes: card.notes
      });
      
      return parseCardFields(result);
    };
    
    // 删除角色卡
    const deleteCharacterCard = async (cardId: string): Promise<void> => {
      await invoke('delete_character_card', { cardId });
    };
    
    // 搜索角色卡
    const searchCharacterCards = async (keyword: string): Promise<CharacterCard[]> => {
      if (!currentBook.value) return [];
      
      const cards = await invoke<CharacterCard[]>('search_character_cards', {
        bookId: currentBook.value.id,
        keyword
      });
      
      return cards.map(parseCardFields);
    };
    
    // ============ 大纲功能 ============
    
    // 保存大纲
    const saveOutline = async (outline: Partial<Outline>): Promise<Outline> => {
      if (!currentBook.value) throw new Error('没有打开的书籍');
      
      const result = await invoke<Outline>('save_outline', {
        bookId: outline.book_id,
        volumeId: outline.volume_id,
        chapterId: outline.chapter_id,
        outlineType: outline.outline_type,
        content: outline.content
      });
      
      return result;
    };
    
    // 获取大纲
    const getOutline = async (outlineId: string): Promise<Outline> => {
      const result = await invoke<Outline>('get_outline', { outlineId });
      return result;
    };
    
    // 按层级获取大纲
    const getOutlineByLevel = async (
      volumeId?: string,
      chapterId?: string,
      outlineType: 'coarse' | 'fine' = 'coarse'
    ): Promise<Outline | null> => {
      if (!currentBook.value) return null;
      
      const result = await invoke<Outline | null>('get_outline_by_level', {
        bookId: currentBook.value.id,
        volumeId,
        chapterId,
        outlineType
      });
      
      return result;
    };
    
    // 获取书籍级大纲
    const listBookOutlines = async (): Promise<Outline[]> => {
      if (!currentBook.value) return [];
      
      const outlines = await invoke<Outline[]>('list_book_outlines', {
        bookId: currentBook.value.id
      });
      
      return outlines;
    };
    
    // 获取卷级大纲
    const listVolumeOutlines = async (volumeId: string): Promise<Outline[]> => {
      if (!currentBook.value) return [];
      
      const outlines = await invoke<Outline[]>('list_volume_outlines', {
        bookId: currentBook.value.id,
        volumeId
      });
      
      return outlines;
    };
    
    // 获取章节级大纲
    const listChapterOutlines = async (chapterId: string): Promise<Outline[]> => {
      if (!currentBook.value) return [];
      
      const outlines = await invoke<Outline[]>('list_chapter_outlines', {
        bookId: currentBook.value.id,
        chapterId
      });
      
      return outlines;
    };
    
    // 删除大纲
    const deleteOutline = async (outlineId: string): Promise<void> => {
      await invoke('delete_outline', { outlineId });
    };

  return {
    // State
    currentBook,
    currentChapterId,
    chatMessages,
    isSaving,
    lastSavedAt,
    booksList,
    hasUnsavedChanges,
    // Getters
    volumes,
    chapters,
    getChaptersByVolume,
    currentChapter,
    currentContent,
    wordCount,
    totalWordCount,
    chapterContents,
    // Actions
    createBook,
    loadBook,
    saveBook,
    selectChapter,
    createChapter,
    deleteChapter,
    updateChapterTitle,
    createVolume,
    deleteVolume,
    updateVolumeTitle,
    toggleVolumeCollapse,
    updateBooksList,
    deleteBook,
    startAutoSave,
    stopAutoSave,
    addMessage,
    clearMessages,
    // 章节快照
    currentChapterSnapshots,
    createChapterSnapshot,
    listChapterSnapshots,
    restoreChapterSnapshot,
    deleteChapterSnapshot,
    compareSnapshots,
    // 自动快照
    markAsModified,
    clearModified,
    createAutoSnapshot,
    createImmediateSnapshot,
    // 角色卡
    listCharacterCards,
    createCharacterCard,
    updateCharacterCard,
    deleteCharacterCard,
    searchCharacterCards,
    // 大纲
    saveOutline,
    getOutline,
    getOutlineByLevel,
    listBookOutlines,
    listVolumeOutlines,
    listChapterOutlines,
    deleteOutline
  };
});
