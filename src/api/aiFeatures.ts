import { invoke } from '@tauri-apps/api/core';
import type { AIConfig, ChapterSummary } from '../types';

/**
 * 生成章节摘要
 */
export async function generateChapterSummary(
  chapterId: string,
  chapterTitle: string,
  content: string,
  config: AIConfig
): Promise<ChapterSummary> {
  return await invoke('generate_chapter_summary', {
    chapterId,
    chapterTitle,
    content,
    config: {
      provider: config.provider,
      api_key: config.apiKey,
      api_url: config.apiUrl,
      model: config.model,
      temperature: config.temperature,
      max_tokens: config.maxTokens
    }
  });
}

/**
 * 加载章节摘要
 */
export async function loadChapterSummary(chapterId: string): Promise<ChapterSummary | null> {
  return await invoke('load_chapter_summary', { chapterId });
}
