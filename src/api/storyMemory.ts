import { invoke } from '@tauri-apps/api/core';
import type { StoryMemory, StoryMemoryUpdateResult } from '../types';

/** 获取故事记忆完整数据 */
export async function getStoryMemory(bookId: string): Promise<StoryMemory | null> {
  return await invoke('get_story_memory', { bookId });
}

/** 获取故事记忆格式化文本 */
export async function getStoryMemoryText(bookId: string): Promise<string> {
  return await invoke('get_story_memory_text', { bookId });
}

/** 手动更新故事记忆（使用缓存） */
export async function updateStoryMemory(bookId: string): Promise<StoryMemoryUpdateResult> {
  return await invoke('update_story_memory', { bookId });
}

/** 强制重新生成故事记忆（清除所有缓存） */
export async function forceRegenerateStoryMemory(bookId: string): Promise<StoryMemoryUpdateResult> {
  return await invoke('force_regenerate_story_memory', { bookId });
}
