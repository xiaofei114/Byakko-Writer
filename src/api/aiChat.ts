import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AIConfig, AIChatMessage, ChatSession } from '../types';

export interface StreamEvent {
  sessionId: string;
  chunk: string;
  isComplete: boolean;
  isToolCall: boolean;
  toolName?: string;
  toolDisplayName?: string;
  toolParams?: Record<string, any>;
}

export interface ChatStreamCallbacks {
  onChunk: (chunk: string) => void;
  onToolCall?: (toolName: string, toolDisplayName?: string, toolParams?: Record<string, any>) => void;
  onComplete: (sessionId?: string) => void;
  onError: (error: string) => void;
}

/**
 * 发送流式 AI 消息（支持工具调用）
 * 
 * @param params 聊天参数
 * @param callbacks 流式回调
 * @returns 取消监听的函数
 */
export async function sendChatMessageStream(
  params: {
    sessionId?: string;
    bookId: string;
    chapterId?: string;
    message: string;
    config: AIConfig;
  },
  callbacks: ChatStreamCallbacks
): Promise<UnlistenFn> {
  let isCompleted = false;
  
  // 先设置事件监听
  const unlisten = await listen<StreamEvent>('ai-chat-stream', (event) => {
    const data = event.payload;

    if (data.isComplete) {
      isCompleted = true;
      callbacks.onComplete(data.sessionId);
    } else if (data.isToolCall && data.toolName) {
      callbacks.onToolCall?.(data.toolName, data.toolDisplayName, data.toolParams);
    } else {
      callbacks.onChunk(data.chunk);
    }
  });

  // 然后发送请求（不等待返回，因为流式响应通过事件传递）
  invoke('send_chat_message_stream', {
    sessionId: params.sessionId,
    bookId: params.bookId,
    chapterId: params.chapterId,
    message: params.message,
    config: {
      provider: params.config.provider,
      api_key: params.config.apiKey,
      api_url: params.config.apiUrl,
      model: params.config.model,
      temperature: params.config.temperature,
      max_tokens: params.config.maxTokens
    }
  }).catch((error) => {
    if (!isCompleted) {
      callbacks.onError(String(error));
    }
  });

  return unlisten;
}

/**
 * 获取聊天历史
 * 
 * @param sessionId 会话ID
 * @param limit 限制数量（默认 50）
 * @returns 消息列表
 */
export async function getChatHistory(
  sessionId: string,
  limit: number = 50
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
