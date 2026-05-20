import { invoke } from '@tauri-apps/api/core';
import type { AIConfig, ChapterSummary, AIChatMessage, ChatSession } from '../types';

// ==================== AI 摘要相关 API ====================

/**
 * 生成章节摘要
 * 
 * @param chapterId 章节ID
 * @param chapterTitle 章节标题
 * @param content 章节内容
 * @param config AI 配置
 * @returns 生成的章节摘要
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
 * 
 * @param chapterId 章节ID
 * @returns 章节摘要（如果不存在返回 null）
 */
export async function loadChapterSummary(chapterId: string): Promise<ChapterSummary | null> {
  return await invoke('load_chapter_summary', { chapterId });
}

/**
 * 确认/更新章节摘要
 * 
 * @param chapterId 章节ID
 * @param summary 摘要内容
 */
export async function confirmChapterSummary(
  chapterId: string,
  summary: {
    shortSummary: string;
    longSummary: string;
    tags: string[];
    characters: string[];
    locations: string[];
    events: string[];
  }
): Promise<void> {
  return await invoke('confirm_chapter_summary', {
    chapterId,
    shortSummary: summary.shortSummary,
    longSummary: summary.longSummary,
    tags: summary.tags,
    characters: summary.characters,
    locations: summary.locations,
    events: summary.events
  });
}

/**
 * 批量生成书籍所有章节的摘要
 * 
 * @param bookId 书籍ID
 * @param config AI 配置
 * @returns 生成的摘要数量
 */
export async function batchGenerateSummaries(
  bookId: string,
  config: AIConfig
): Promise<number> {
  return await invoke('batch_generate_summaries', {
    bookId,
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

// ==================== AI 聊天相关 API ====================

export interface AIChatResponse {
  sessionId: string;
  message: AIChatMessage;
}

/**
 * 发送带上下文的 AI 消息
 * 
 * @param params 聊天参数
 * @returns AI 回复和会话ID
 */
export async function sendChatMessageWithContext(params: {
  sessionId?: string;
  bookId: string;
  bookTitle: string;
  chapterId?: string;
  chapterTitle?: string;
  message: string;
  useRag?: boolean;
  config: AIConfig;
}): Promise<AIChatResponse> {
  return await invoke('send_chat_message_with_context', {
    sessionId: params.sessionId,
    bookId: params.bookId,
    bookTitle: params.bookTitle,
    chapterId: params.chapterId,
    chapterTitle: params.chapterTitle,
    message: params.message,
    useRag: params.useRag ?? true,
    config: {
      provider: params.config.provider,
      api_key: params.config.apiKey,
      api_url: params.config.apiUrl,
      model: params.config.model,
      temperature: params.config.temperature,
      max_tokens: params.config.maxTokens
    }
  });
}

/**
 * 获取聊天历史
 * 
 * @param sessionId 会话ID
 * @param limit 限制数量（默认 10）
 * @returns 消息列表
 */
export async function getChatHistory(
  sessionId: string,
  limit: number = 10
): Promise<AIChatMessage[]> {
  return await invoke('get_chat_history', { sessionId, limit });
}

/**
 * 获取书籍的所有会话列表
 * 
 * @param bookId 书籍ID
 * @returns 会话列表
 */
export async function getChatSessions(bookId: string): Promise<ChatSession[]> {
  return await invoke('get_chat_sessions', { bookId });
}

/**
 * 删除会话
 * 
 * @param sessionId 会话ID
 */
export async function deleteChatSession(sessionId: string): Promise<void> {
  return await invoke('delete_chat_session', { sessionId });
}

/**
 * 清空书籍的所有对话历史
 * 
 * @param bookId 书籍ID
 */
export async function clearBookChatHistory(bookId: string): Promise<void> {
  return await invoke('clear_book_chat_history', { bookId });
}
