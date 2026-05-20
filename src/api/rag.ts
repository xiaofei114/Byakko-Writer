import { invoke } from '@tauri-apps/api/core';
import type { SearchResult, AIConfig } from '../types';

/**
 * 构建 RAG 上下文
 * 
 * 基于用户查询检索相关内容并构建完整的上下文
 * 
 * @param bookId 书籍ID
 * @param query 用户查询
 * @param currentChapterId 当前章节ID（可选）
 * @param aiConfig AI配置（用于Embedding）
 * @returns 构建好的上下文字符串
 */
export async function buildRagContext(
  bookId: string,
  query: string,
  currentChapterId?: string,
  aiConfig?: AIConfig
): Promise<string> {
  try {
    const context = await invoke<string>('build_rag_context', {
      bookId,
      query,
      currentChapterId,
      aiConfig
    });
    return context;
  } catch (error) {
    console.error('构建RAG上下文失败:', error);
    throw error;
  }
}

/**
 * 索引章节内容
 * 
 * 将章节内容提取摘要并索引到向量数据库
 * 
 * @param bookId 书籍ID
 * @param chapterId 章节ID
 * @param chapterTitle 章节标题
 * @param content 章节内容
 */
export async function indexChapter(
  bookId: string,
  chapterId: string,
  chapterTitle: string,
  content: string
): Promise<void> {
  try {
    await invoke('index_chapter_command', {
      bookId,
      chapterId,
      chapterTitle,
      content
    });
  } catch (error) {
    console.error('索引章节失败:', error);
    throw error;
  }
}

/**
 * 搜索相关内容
 * 
 * 基于查询文本搜索相似的文档
 * 
 * @param bookId 书籍ID
 * @param query 查询文本
 * @param docTypes 文档类型过滤（可选）
 * @param limit 返回结果数量限制
 * @returns 搜索结果列表
 */
export async function searchRelatedContent(
  bookId: string,
  query: string,
  docTypes?: string[],
  limit: number = 5
): Promise<SearchResult[]> {
  try {
    const results = await invoke<SearchResult[]>('search_related_content', {
      bookId,
      query,
      docTypes,
      limit
    });
    return results;
  } catch (error) {
    console.error('搜索相关内容失败:', error);
    throw error;
  }
}

/**
 * 智能 AI 对话（带 RAG 上下文）
 * 
 * 先构建 RAG 上下文，然后发送给 AI
 * 
 * @param bookId 书籍ID
 * @param message 用户消息
 * @param currentChapterId 当前章节ID（可选）
 * @param aiConfig AI 配置
 * @returns AI 回复
 */
export async function sendRagMessage(
  bookId: string,
  message: string,
  currentChapterId?: string,
  aiConfig?: {
    provider: string;
    apiKey: string;
    apiUrl: string;
    model: string;
    temperature: number;
    maxTokens: number;
  }
): Promise<string> {
  // 1. 构建 RAG 上下文
  const context = await buildRagContext(bookId, message, currentChapterId);
  
  // 2. 调用 AI API（使用现有的 ai.ts 中的函数）
  // 这里需要导入 sendMessage 函数
  const { sendMessage } = await import('./ai');
  
  if (!aiConfig) {
    throw new Error('AI 配置未提供');
  }
  
  // 3. 发送带上下文的请求
  const response = await sendMessage(
    context,
    [], // 历史对话为空，因为上下文已经包含在消息中
    {
      provider: aiConfig.provider,
      apiKey: aiConfig.apiKey,
      apiUrl: aiConfig.apiUrl,
      model: aiConfig.model,
      temperature: aiConfig.temperature,
      maxTokens: aiConfig.maxTokens
    }
  );
  
  return response;
}

/**
 * 批量索引多个章节
 * 
 * @param bookId 书籍ID
 * @param chapters 章节列表
 */
export async function batchIndexChapters(
  bookId: string,
  chapters: Array<{
    id: string;
    title: string;
    content: string;
  }>
): Promise<void> {
  for (const chapter of chapters) {
    try {
      await indexChapter(bookId, chapter.id, chapter.title, chapter.content);
      // 添加小延迟避免 API 限流
      await new Promise(resolve => setTimeout(resolve, 100));
    } catch (error) {
      console.error(`索引章节 ${chapter.id} 失败:`, error);
      // 继续处理其他章节
    }
  }
}

/**
 * 获取角色信息
 * 
 * 搜索与角色相关的内容
 * 
 * @param bookId 书籍ID
 * @param characterName 角色名称
 * @returns 角色相关信息列表
 */
export async function searchCharacterInfo(
  bookId: string,
  characterName: string
): Promise<SearchResult[]> {
  return searchRelatedContent(
    bookId,
    `角色：${characterName}`,
    ['character', 'chapter_summary'],
    10
  );
}

/**
 * 获取剧情回顾
 * 
 * 搜索与当前剧情相关的内容
 * 
 * @param bookId 书籍ID
 * @param plotPoint 剧情要点
 * @returns 相关章节摘要列表
 */
export async function getPlotRecap(
  bookId: string,
  plotPoint: string
): Promise<SearchResult[]> {
  return searchRelatedContent(
    bookId,
    plotPoint,
    ['chapter_summary'],
    5
  );
}
