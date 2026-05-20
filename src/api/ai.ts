import { invoke } from '@tauri-apps/api/core';
import type { ChatMessage, AIConfig } from '../types';

export async function sendMessage(
  message: string,
  history: ChatMessage[],
  config: AIConfig
): Promise<string> {
  try {
    const response = await invoke<string>('send_ai_message', {
      message,
      history: history.map(m => ({ role: m.role, content: m.content })),
      config: {
        provider: config.provider,
        api_key: config.apiKey,
        api_url: config.apiUrl,
        model: config.model,
        temperature: config.temperature,
        max_tokens: config.maxTokens
      }
    });
    return response;
  } catch (error) {
    console.error('AI请求失败:', error);
    throw error;
  }
}
