import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AIConfig, AIChatMessage, ChatSession } from '../types';

export interface StreamEvent {
  sessionId: string;
  chunk: string;
  isComplete: boolean;
  isToolCall: boolean;
  toolName?: string;
  toolDisplayName?: string;
  toolParams?: Record<string, any>;
  sessionStarted?: boolean;
}

// Agent 阶段事件
export interface AgentPhaseEvent {
  type: 'phase_start' | 'phase_complete' | 'tool_call' | 'tool_result';
  sessionId: string;
  phase?: 'intent' | 'tool' | 'writing' | 'outlining' | 'polishing';
  message?: string;
  result?: string;
}

export interface ChatStreamCallbacks {
  onChunk: (chunk: string) => void;
  onToolCall?: (toolName: string, toolDisplayName?: string, toolParams?: Record<string, any>) => void;
  onComplete: (sessionId?: string) => void;
  onError: (error: string) => void;
  onAgentPhase?: (event: AgentPhaseEvent) => void;
  onSessionStarted?: (sessionId: string) => void;
}

/**
 * 发送流式 AI 消息（支持工具调用和Agent阶段显示）
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
): Promise<() => void> {
  let isCompleted = false;
  
  // 设置流式事件监听
  const unlistenStream = await listen<StreamEvent>('ai-chat-stream', (event) => {
    const data = event.payload;

    // sessionStarted 表示会话已创建，更新 sessionId 但不结束对话
    if (data.sessionStarted && data.sessionId) {
      callbacks.onSessionStarted?.(data.sessionId);
      return;
    }

    if (data.isComplete) {
      isCompleted = true;
      callbacks.onComplete(data.sessionId);
    } else if (data.isToolCall && data.toolName) {
      callbacks.onToolCall?.(data.toolName, data.toolDisplayName, data.toolParams);
    } else {
      callbacks.onChunk(data.chunk);
    }
  });

  // 设置Agent阶段事件监听
  const unlistenPhase = await listen<AgentPhaseEvent>('ai-agent-phase', (event) => {
    callbacks.onAgentPhase?.(event.payload);
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

  // 返回统一的取消监听函数
  return () => {
    unlistenStream();
    unlistenPhase();
  };
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

/**
 * 更新消息的 polish_handled 状态
 *
 * @param messageId 消息ID
 * @param handled 是否已处理
 */
export async function updateMessagePolishHandled(messageId: string, handled: boolean): Promise<void> {
  return await invoke('update_message_polish_handled', { messageId, handled });
}

export async function updateLineEditHandledStatus(messageId: string, handledStatus: string): Promise<void> {
  return await invoke('update_line_edit_handled_status', { messageId, handledStatus });
}

/**
 * 发送润色请求（独立流程，不参与对话上下文）
 *
 * @param params 润色参数
 * @param callbacks 流式回调
 * @returns 取消监听的函数
 */
export async function sendPolishRequest(
  params: {
    sessionId?: string;
    bookId: string;
    chapterId?: string;
    originalText: string;
    instruction: string;
    config: AIConfig;
  },
  callbacks: ChatStreamCallbacks
): Promise<() => void> {
  let isCompleted = false;

  // 设置流式事件监听
  const unlistenStream = await listen<StreamEvent>('ai-chat-stream', (event) => {
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

  // 发送润色请求
  invoke('send_polish_request', {
    sessionId: params.sessionId,
    bookId: params.bookId,
    chapterId: params.chapterId,
    originalText: params.originalText,
    instruction: params.instruction,
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

  // 返回取消监听函数
  return () => {
    unlistenStream();
  };
}
