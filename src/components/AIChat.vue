<script setup lang="ts">
import { ref, nextTick, onMounted, onUnmounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { useBookStore } from '../stores/book';
import { useConfigStore } from '../stores/config';
import { sendChatMessageStream, sendPolishRequest, getChatSessions, deleteChatSession, getChatHistory, updateMessagePolishHandled, type AgentPhaseEvent } from '../api/aiChat';
import type { AIChatMessage, ChatSession } from '../types';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { listen } from '@tauri-apps/api/event';
import { marked } from 'marked';
import { Delete, Close, UserFilled, Cpu, Promotion, Plus, ArrowDown, Check, Close as CloseIcon } from '@element-plus/icons-vue';
import { ElMessage, ElMessageBox, ElNotification } from 'element-plus';
import { getPolishRequest, clearPolishRequest } from '../utils/eventBus';

// 配置 marked
marked.setOptions({
  breaks: true,
  gfm: true
});

// 渲染 Markdown
const renderMarkdown = (content: string) => {
  if (!content) return '';
  // 过滤掉工具调用标记
  const cleanedContent = filterToolCalls(content);
  return marked.parse(cleanedContent);
};

// 过滤润色提示词
const filterToolCalls = (content: string): string => {
  const polishResults: string[] = [];
  let temp = content.replace(/<polish_result>[\s\S]*?<\/polish_result>/g, (match) => {
    polishResults.push(match);
    return `___POLISH_RESULT_${polishResults.length - 1}___`;
  });
  let cleaned = temp;
  cleaned = cleaned.replace(/请直接返回润色后的文本[\s\S]*$/g, '');
  cleaned = cleaned.replace(/必须在回复末尾使用以下格式[\s\S]*$/g, '');
  polishResults.forEach((match, i) => {
    cleaned = cleaned.replace(`___POLISH_RESULT_${i}___`, match);
  });
  return cleaned.trim();
};

// 解析润色结果
const parsePolishResult = (content: string): { hasResult: boolean; displayContent: string; originalText?: string; polishedText?: string } => {
  // 匹配新格式：<polish_result>原文：xxx润色后：yyy</polish_result>
  const match = content.match(/<polish_result>\s*原文：\s*([\s\S]*?)\s*润色后：\s*([\s\S]*?)\s*<\/polish_result>/);
  if (match) {
    const originalText = match[1].trim();
    const polishedText = match[2].trim();
    // 移除润色结果标记，显示简洁版本
    const displayContent = content.replace(/<polish_result>[\s\S]*?<\/polish_result>/g, '').trim();
    return { hasResult: true, displayContent, originalText, polishedText };
  }
  // 兼容旧格式：<polish_result>润色后：xxx</polish_result>
  const oldMatch = content.match(/<polish_result>\s*润色后：\s*([\s\S]*?)\s*<\/polish_result>/);
  if (oldMatch) {
    const polishedText = oldMatch[1].trim();
    const displayContent = content.replace(/<polish_result>[\s\S]*?<\/polish_result>/g, '').trim();
    return { hasResult: true, displayContent, originalText: currentPolishOriginalText, polishedText };
  }
  return { hasResult: false, displayContent: content };
};

// 获取显示内容（处理润色格式）
const getDisplayContent = (content: string): string => {
  const polishResult = parsePolishResult(content);
  if (polishResult.hasResult) {
    return polishResult.displayContent;
  }
  return content;
};

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const bookStore = useBookStore();
const configStore = useConfigStore();

const inputMessage = ref('');
const isLoading = ref(false);
const isStreaming = ref(false);
const currentAgentPhase = ref<string>('');
const messagesContainer = ref<HTMLDivElement>();
const currentSessionId = ref<string | undefined>(undefined);
const sessions = ref<ChatSession[]>([]);
const showSessionList = ref(false);
const messages = ref<AIChatMessage[]>([]);
const historyMessageIds = ref<Set<string>>(new Set()); // 历史消息ID，不显示操作按钮
const currentStreamContent = ref('');
const streamUnlisten = ref<UnlistenFn | null>(null);
const dataChangeUnlisten = ref<UnlistenFn | null>(null); // 数据变更事件监听器

// 工具调用相关状态
const isInToolCallRound = ref(false); // 标记是否在一轮工具调用中
const currentToolCallNames = ref<string[]>([]); // 当前轮次的工具名称列表



// 跟踪每个消息中已保存的大纲类型 (messageId -> Set<outlineType>)
const savedOutlines = ref<Map<string, Set<string>>>(new Map());

// 打字机效果相关
const typewriterBuffer = ref(''); // 待显示的字符缓冲区
const typewriterTimer = ref<number | null>(null);
const TYPEWRITER_INTERVAL = 30; // 正常速度：每个字符显示的间隔（毫秒）
const TYPEWRITER_FAST_INTERVAL = 5; // 加速：对话完成后的显示速度

// 启动打字机效果
const startTypewriter = (fastMode = false) => {
  // 如果已经在运行且不需要切换速度，直接返回
  if (typewriterTimer.value && !fastMode) {
    return;
  }

  // 如果已经在运行，先停止（用于切换速度）
  if (typewriterTimer.value) {
    clearInterval(typewriterTimer.value);
    typewriterTimer.value = null;
  }
  
  const interval = fastMode ? TYPEWRITER_FAST_INTERVAL : TYPEWRITER_INTERVAL;
  
  const typeNextChar = () => {
    if (typewriterBuffer.value.length === 0) {
      // 缓冲区为空，停止定时器
      if (typewriterTimer.value) {
        clearInterval(typewriterTimer.value);
        typewriterTimer.value = null;
      }
      return;
    }
    
    // 取出一个字符显示
    const char = typewriterBuffer.value.charAt(0);
    typewriterBuffer.value = typewriterBuffer.value.slice(1);
    currentStreamContent.value += char;
    
    // 更新最后一条消息
    const lastMsg = messages.value[messages.value.length - 1];
    if (lastMsg && lastMsg.role === 'assistant') {
      lastMsg.content = currentStreamContent.value;
    }
    scrollToBottom();
  };
  
  typewriterTimer.value = window.setInterval(typeNextChar, interval);
};

// 加速显示剩余内容（对话完成时使用）
const speedUpTypewriter = () => {
  if (typewriterBuffer.value.length > 0) {
    startTypewriter(true); // 加速模式
  }
};

// 停止打字机效果并立即显示所有剩余内容（工具调用/错误时使用）
const stopTypewriter = () => {
  if (typewriterTimer.value) {
    clearInterval(typewriterTimer.value);
    typewriterTimer.value = null;
  }
  // 立即显示剩余内容
  if (typewriterBuffer.value.length > 0) {
    currentStreamContent.value += typewriterBuffer.value;
    typewriterBuffer.value = '';
    const lastMsg = messages.value[messages.value.length - 1];
    if (lastMsg && lastMsg.role === 'assistant') {
      lastMsg.content = currentStreamContent.value;
    }
  }
};

const scrollToBottom = async () => {
  await nextTick();
  if (messagesContainer.value) {
    messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight;
  }
};

// 加载会话列表
const loadSessions = async () => {
  if (!bookStore.currentBook) return;
  try {
    sessions.value = await getChatSessions(bookStore.currentBook.id);
  } catch (error) {
    console.error('加载会话列表失败:', error);
  }
};

// 新建会话
const createNewSession = () => {
  currentSessionId.value = undefined;
  messages.value = [];
  historyMessageIds.value = new Set();
  ElMessage.success('已创建新会话');
};

// 切换会话
const switchSession = async (session: ChatSession) => {
  currentSessionId.value = session.sessionId;
  await loadSessionMessages(session.sessionId);
  showSessionList.value = false;
};

// 加载会话消息
const loadSessionMessages = async (sessionId: string) => {
  try {
    const history = await getChatHistory(sessionId, 50);
    messages.value = history;
    // 标记为历史消息（不显示操作按钮）
    historyMessageIds.value = new Set(history.map(m => m.id));
    // 恢复大纲保存状态（内存态在刷新后丢失，需从数据库恢复）
    await restoreOutlineSavedState();
    await scrollToBottom();
  } catch (error) {
    console.error('加载会话消息失败:', error);
  }
};

// 恢复大纲保存状态：检查数据库里是否已存在对应大纲
const restoreOutlineSavedState = async () => {
  savedOutlines.value.clear();
  for (const msg of messages.value) {
    if (msg.role === 'outline') {
      const data = parseOutlineMsg(msg);
      if (data?.chapterId) {
        try {
          const existing = await bookStore.getOutlineByLevel(
            undefined,
            data.chapterId,
            data.outlineType as 'coarse' | 'fine'
          );
          if (existing && existing.content) {
            markOutlineSaved(msg.id, data.outlineType);
          }
        } catch { /* 大纲不存在，不标记 */ }
      }
    }
  }
};

// 删除会话
const handleDeleteSession = async (session: ChatSession, event: Event) => {
  event.stopPropagation();
  try {
    await ElMessageBox.confirm(`确定删除会话 "${session.title}" 吗？`, '确认删除', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning'
    });
    await deleteChatSession(session.sessionId);
    if (currentSessionId.value === session.sessionId) {
      currentSessionId.value = undefined;
      messages.value = [];
    }
    await loadSessions();
    ElMessage.success('会话已删除');
  } catch (error) {
    // 用户取消
  }
};

const handleSend = async () => {
  if (!inputMessage.value.trim() || isLoading.value || !bookStore.currentBook) return;

  const userMessage = inputMessage.value.trim();
  inputMessage.value = '';

  // 添加用户消息到本地
  const userMsg: AIChatMessage = {
    id: Date.now().toString(),
    bookId: bookStore.currentBook.id,
    chapterId: bookStore.currentChapterId || undefined,
    sessionId: currentSessionId.value || '',
    role: 'user',
    content: userMessage,
    timestamp: Date.now()
  };

  messages.value.push(userMsg);
  await scrollToBottom();

  // 准备接收 AI 回复 — 先清理上一次的残留状态
  stopTypewriter();
  typewriterBuffer.value = '';
  currentStreamContent.value = '';

  isLoading.value = true;
  isStreaming.value = true;

  // 添加一个空的 AI 消息用于流式填充
  const aiMsgId = (Date.now() + 1).toString();
  const aiMsg: AIChatMessage = {
    id: aiMsgId,
    bookId: bookStore.currentBook.id,
    sessionId: currentSessionId.value || '',
    role: 'assistant',
    content: '',
    timestamp: Date.now()
  };
  messages.value.push(aiMsg);

  try {
    // 清理上一次的流监听（防止 listener 堆叠）
    if (streamUnlisten.value) {
      streamUnlisten.value();
      streamUnlisten.value = null;
    }
    if (dataChangeUnlisten.value) {
      dataChangeUnlisten.value();
      dataChangeUnlisten.value = null;
    }

    // 监听数据变更事件（工具调用后刷新数据）
    dataChangeUnlisten.value = await listen('ai-data-changed', (event) => {
      const data = event.payload as { toolName: string; bookId: string; chapterId?: string };
      // 如果变更的是当前书籍的数据，刷新书籍数据
      if (data.bookId === bookStore.currentBook?.id) {
        // 重新加载书籍数据以获取最新的大纲、角色卡等
        bookStore.loadBook(data.bookId);
      }
    });
    
    streamUnlisten.value = await sendChatMessageStream(
      {
        sessionId: currentSessionId.value,
        bookId: bookStore.currentBook.id,
        chapterId: bookStore.currentChapterId || undefined,
        message: userMessage,
        config: configStore.aiConfig
      },
      {
        onChunk: (chunk: string) => {
          // 如果之前是工具调用轮，现在收到内容了，需要创建新的 AI 消息
          if (isInToolCallRound.value) {
            isInToolCallRound.value = false;

            // 为 AI 的回复创建新的消息气泡
            currentStreamContent.value = '';
            const newMessage: AIChatMessage = {
              id: 'ai_' + Date.now(),
              bookId: bookStore.currentBook?.id || '',
              chapterId: bookStore.currentChapterId || undefined,
              sessionId: currentSessionId.value || '',
              role: 'assistant',
              content: '',
              timestamp: Date.now()
            };
            messages.value.push(newMessage);
          }

          // 将内容添加到缓冲区
          typewriterBuffer.value += chunk;

          // 启动打字机效果（如果未启动）
          startTypewriter();
          scrollToBottom();
        },
        onToolCall: (toolName: string, toolDisplayName?: string, toolParams?: Record<string, any>) => {
          const displayName = toolDisplayName || toolName;
          const displayText = buildToolDisplayText(toolName, displayName, toolParams);
          
          // 如果是这一轮的第一个工具调用，先停止打字机并准备新消息
          if (!isInToolCallRound.value) {
            isInToolCallRound.value = true;
            currentToolCallNames.value = [];
            
            // 停止打字机效果，立即显示当前缓冲区内容
            stopTypewriter();
            
            // 检查最后一条 AI 消息是否为空，如果为空则删除
            const lastMsg = messages.value[messages.value.length - 1];
            if (lastMsg && lastMsg.role === 'assistant' && !lastMsg.content.trim()) {
              messages.value.pop();
            }
          }
          
          // 收集工具显示文本
          currentToolCallNames.value.push(displayText);
          
          // 更新或创建工具调用消息
          const lastMsg = messages.value[messages.value.length - 1];
          if (lastMsg && lastMsg.role === 'tool') {
            // 更新现有工具消息 - 用特殊格式存储数组
            lastMsg.content = JSON.stringify(currentToolCallNames.value);
          } else {
            // 创建新的工具消息
            const toolMsg: AIChatMessage = {
              id: 'tool_' + Date.now(),
              bookId: bookStore.currentBook?.id || '',
              chapterId: bookStore.currentChapterId || undefined,
              sessionId: currentSessionId.value || '',
              role: 'tool',
              content: JSON.stringify(currentToolCallNames.value),
              toolName: toolName,
              toolStatus: 'calling',
              timestamp: Date.now()
            };
            messages.value.push(toolMsg);
          }
          
          scrollToBottom();
        },
        onAgentPhase: (event: AgentPhaseEvent) => {
          if (event.type === 'phase_start' && event.phase) {
            currentAgentPhase.value = event.phase;
          } else if (event.type === 'phase_complete') {
            currentAgentPhase.value = '';
          }
        },
        onComplete: (sessionId?: string) => {
          if (sessionId) {
            currentSessionId.value = sessionId;
          }
          isLoading.value = false;
          isStreaming.value = false;
          currentAgentPhase.value = '';
          // 重置工具调用状态
          isInToolCallRound.value = false;
          currentToolCallNames.value = [];
          // 对话完成，加速显示剩余内容
          speedUpTypewriter();
          // 清理监听器
          if (streamUnlisten.value) {
            streamUnlisten.value();
            streamUnlisten.value = null;
          }
          if (dataChangeUnlisten.value) {
            dataChangeUnlisten.value();
            dataChangeUnlisten.value = null;
          }
          loadSessions(); // 刷新会话列表
        },
        onError: (error: string) => {
          console.error('AI 流式请求失败:', error);
          // 停止打字机效果
          stopTypewriter();
          const lastMsg = messages.value[messages.value.length - 1];
          if (lastMsg && lastMsg.role === 'assistant') {
            lastMsg.content = '抱歉，请求失败: ' + error;
          }
          isLoading.value = false;
          isStreaming.value = false;
          currentAgentPhase.value = '';
          // 重置工具调用状态
          isInToolCallRound.value = false;
          currentToolCallNames.value = [];
          // 清理监听器
          if (streamUnlisten.value) {
            streamUnlisten.value();
            streamUnlisten.value = null;
          }
          if (dataChangeUnlisten.value) {
            dataChangeUnlisten.value();
            dataChangeUnlisten.value = null;
          }
          ElNotification({ title: 'AI 请求失败', message: error, type: 'error', duration: 0 });
        }
      }
    );
  } catch (error) {
    const lastMsg = messages.value[messages.value.length - 1];
    if (lastMsg && lastMsg.role === 'assistant') {
      lastMsg.content = '抱歉，发送消息失败，请检查AI配置。';
    }
    isLoading.value = false;
    isStreaming.value = false;
  }
};

// 监听润色请求
const polishUnwatch = watch(getPolishRequest(), (request: import('../utils/eventBus').PolishRequest | null) => {
  if (request) {
    handlePolishRequest(request);
    clearPolishRequest();
  }
});

// 存储当前润色请求的原文
let currentPolishOriginalText = '';

// 处理润色请求
const handlePolishRequest = async (request: import('../utils/eventBus').PolishRequest) => {
  // 保存原文供后续使用
  currentPolishOriginalText = request.text;

  if (!bookStore.currentBook) {
    ElMessage.error('没有打开的书籍');
    return;
  }

  // 清理上一次的流监听（防止 listener 堆叠）
  if (streamUnlisten.value) {
    streamUnlisten.value();
    streamUnlisten.value = null;
  }
  // 清理上一次的打字机状态
  stopTypewriter();
  typewriterBuffer.value = '';
  currentStreamContent.value = '';

  isLoading.value = true;
  isStreaming.value = true;

  // 创建 AI 消息气泡
  const aiMsg: AIChatMessage = {
    id: 'ai_' + Date.now(),
    bookId: bookStore.currentBook.id,
    chapterId: bookStore.currentChapterId || undefined,
    sessionId: currentSessionId.value || '',
    role: 'assistant',
    content: '',
    timestamp: Date.now()
  };
  messages.value.push(aiMsg);

  try {
    const unlisten = await sendPolishRequest(
      {
        sessionId: currentSessionId.value,
        bookId: bookStore.currentBook.id,
        chapterId: bookStore.currentChapterId || undefined,
        originalText: request.text,
        instruction: '请润色以下文本，使其语言更加流畅自然、节奏紧凑，但保持原有的情节和人物设定不变',
        config: configStore.aiConfig
      },
      {
        onChunk: (chunk: string) => {
          typewriterBuffer.value += chunk;
          startTypewriter();
          scrollToBottom();
        },
        onToolCall: () => {},
        onComplete: (sessionId?: string) => {
          if (sessionId) currentSessionId.value = sessionId;
          isLoading.value = false;
          isStreaming.value = false;
          currentAgentPhase.value = '';
          speedUpTypewriter();
          loadSessions();
          // 清理监听器
          if (streamUnlisten.value) {
            streamUnlisten.value();
            streamUnlisten.value = null;
          }
        },
        onError: (error: string) => {
          isLoading.value = false;
          isStreaming.value = false;
          currentAgentPhase.value = '';
          // 清理监听器
          if (streamUnlisten.value) {
            streamUnlisten.value();
            streamUnlisten.value = null;
          }
          ElNotification({ title: '润色请求失败', message: error, type: 'error', duration: 0 });
        }
      }
    );
    streamUnlisten.value = unlisten;
  } catch (error) {
    isLoading.value = false;
    isStreaming.value = false;
    currentAgentPhase.value = '';
  }
};

// 应用润色结果
const applyPolish = async (msg: AIChatMessage, originalText: string, polishedText: string) => {
  // 标记已处理
  msg.polishHandled = true;

  // 保存到数据库
  try {
    await updateMessagePolishHandled(msg.id, true);
  } catch (e) {
    console.error('保存 polishHandled 状态失败:', e);
  }

  // 发送事件到 WritingArea
  window.dispatchEvent(new CustomEvent('apply-polish', {
    detail: { originalText, polishedText }
  }));
  ElMessage.success('已应用润色结果');
};

// 取消润色
const cancelPolish = async (msg: AIChatMessage) => {
  // 标记已处理
  msg.polishHandled = true;

  // 保存到数据库
  try {
    await updateMessagePolishHandled(msg.id, true);
  } catch (e) {
    console.error('保存 polishHandled 状态失败:', e);
  }

  ElMessage.info('已取消应用');
};

// 检查某个大纲是否已保存
const isOutlineSaved = (msgId: string, outlineType: string): boolean => {
  return savedOutlines.value.get(msgId)?.has(outlineType) || false;
};

// 标记大纲已保存
const markOutlineSaved = (msgId: string, outlineType: string) => {
  if (!savedOutlines.value.has(msgId)) {
    savedOutlines.value.set(msgId, new Set());
  }
  savedOutlines.value.get(msgId)!.add(outlineType);
};

// 解析 write 消息中的创作数据
const parseWriteMsg = (msg: AIChatMessage): { chapterId: string; content: string; description: string } | null => {
  try { return JSON.parse(msg.content); } catch { return null; }
};

// 插入模式
type InsertMode = 'replace' | 'append';
const writeInsertMode = ref<Record<string, InsertMode>>({}); // msgId -> mode

// 执行插入
const applyWrite = async (msg: AIChatMessage) => {
  const data = parseWriteMsg(msg);
  if (!data || !bookStore.currentBook) return;

  const mode = writeInsertMode.value[msg.id] || 'append';

  // 清理内容：去首尾空行，合并连续空行
  const cleanContent = data.content
    .replace(/^[\n\r]+/, '')           // 去开头空行
    .replace(/[\n\r]+$/, '')           // 去结尾空行
    .replace(/\n{2,}/g, '\n');       // 3个及以上连续空行合并为1个

  try {
    if (mode === 'replace') {
      await invoke('save_chapter_content', {
        bookId: bookStore.currentBook.id,
        chapterId: data.chapterId,
        content: cleanContent,
      });
    } else {
      // append: 先读当前内容再拼接保存
      const current = await invoke<string>('load_chapter_content', {
        bookId: bookStore.currentBook.id,
        chapterId: data.chapterId,
      });
      const separator = current && !current.endsWith('\n') ? '\n\n' : '';
      await invoke('save_chapter_content', {
        bookId: bookStore.currentBook.id,
        chapterId: data.chapterId,
        content: current + separator + cleanContent,
      });
    }
    msg.content = JSON.stringify({ ...data, applied: true });
    ElMessage.success('内容已保存');
    await bookStore.loadBook(bookStore.currentBook.id);
  } catch (e) {
    ElMessage.error('保存失败: ' + e);
  }
};

// 取消插入
const cancelWrite = (msg: AIChatMessage) => {
  msg.content = JSON.stringify({ ...parseWriteMsg(msg), applied: true });
  ElMessage.info('已取消');
};

// 检查是否已处理
const isWriteApplied = (msg: AIChatMessage): boolean => {
  try { return JSON.parse(msg.content).applied === true; } catch { return false; }
};

// 应用大纲结果
const applyOutline = async (msg: AIChatMessage, chapterId: string, outlineType: string, content: string) => {
  if (!bookStore.currentBook) return;
  
  try {
    // 保存大纲
    await bookStore.saveOutline({
      book_id: bookStore.currentBook.id,
      chapter_id: chapterId,
      outline_type: outlineType as 'coarse' | 'fine',
      content: content
    });
    
    // 标记该类型大纲已保存
    markOutlineSaved(msg.id, outlineType);
    
    // 发送事件通知大纲编辑器刷新
    console.log('[AIChat] 发送 outline-saved 事件:', { chapterId, outlineType });
    window.dispatchEvent(new CustomEvent('outline-saved', {
      detail: { chapterId, outlineType }
    }));
    
    ElMessage.success(outlineType === 'coarse' ? '粗纲已保存' : '细纲已保存');
  } catch (error) {
    ElMessage.error('保存大纲失败');
  }
};

// 解析工具消息内容为显示文本数组（兼容新旧格式）
const parseToolItems = (content: string): string[] => {
  try {
    const data = JSON.parse(content);
    if (!Array.isArray(data)) return [content];
    return data.map((item: any) => {
      // 新格式：{name, chapterId, ...} → 用 buildToolDisplayText 重建
      if (typeof item === 'object' && item.name) {
        const displayName = item.name; // fallback
        return buildToolDisplayText(item.name, displayName, {
          chapterId: item.chapterId,
          outlineType: item.outlineType,
          description: item.description,
          startLine: item.startLine,
          endLine: item.endLine,
        });
      }
      // 旧格式：纯字符串
      return String(item);
    });
  } catch {
    return [content];
  }
};

// 构建工具调用的用户友好显示文本
const buildToolDisplayText = (toolName: string, displayName: string, params?: Record<string, any>): string => {
  const chapters = bookStore.currentBook?.chapters || [];
  const findChapterTitle = (id: string) => {
    const ch = chapters.find(c => c.id === id);
    return ch ? `「${ch.title}」` : '';
  };

  switch (toolName) {
    case 'query_chapter_summary':
      return params?.chapterId
        ? `查询${findChapterTitle(params.chapterId)}摘要`
        : displayName;
    case 'search_chapter_content': {
      if (!params?.keyword) return displayName;
      const title = params.chapterId ? (findChapterTitle(params.chapterId) || '章节') : '全书';
      const mode = params.regex ? '正则' : '';
      return `在${title}${mode}搜索「${params.keyword}」`;
    }
    case 'query_chapter_content': {
      if (!params?.chapterId) return displayName;
      const title = findChapterTitle(params.chapterId) || `章节`;
      const start = params.startLine as number | undefined;
      const end = params.endLine as number | undefined;
      if (!start && !end) return `查询${title}全文`;
      if (start && start < 0) return `查询${title}后${Math.abs(start)}行`;
      if (start && !end) return `查询${title}第${start}行起`;
      if (!start && end) return `查询${title}前${end}行`;
      return `查询${title}第${start}-${end}行`;
    }
    case 'get_outline':
      return params?.chapterId
        ? `获取${findChapterTitle(params.chapterId)}大纲`
        : displayName;
    case 'get_character_card':
      return params?.name ? `查询角色「${params.name}」详情` : '查询角色详情';
    case 'create_character_card':
      return params?.name ? `创建角色「${params.name}」` : displayName;
    case 'write_chapter': {
      const chTitle = params?.chapterId ? findChapterTitle(params.chapterId) : '章节';
      const desc = params?.description ? `：${params.description}` : '';
      return `创作${chTitle}${desc}`;
    }
    case 'save_outline': {
      const chTitle = params?.chapterId ? findChapterTitle(params.chapterId) : '';
      const typeLabel = params?.outlineType === 'coarse' ? '粗纲' : '细纲';
      return chTitle ? `生成${chTitle}${typeLabel}` : `生成${typeLabel}`;
    }
    default:
      return displayName;
  }
};

// 解析 outline 消息中的大纲数据
const parseOutlineMsg = (msg: AIChatMessage): { chapterId: string; outlineType: string; content: string } | null => {
  try {
    return JSON.parse(msg.content);
  } catch {
    return null;
  }
};

// 取消保存大纲
const cancelOutline = (msg: AIChatMessage) => {
  const outlineData = parseOutlineMsg(msg);
  if (outlineData) {
    markOutlineSaved(msg.id, outlineData.outlineType);
    ElMessage.info('已取消保存大纲');
  }
};

// 组件卸载时清理
// outline-result 事件监听（AI 通过 function calling 生成大纲后触发）
let outlineResultUnlisten: (() => void) | null = null;
let writeResultUnlisten: (() => void) | null = null;
let conflictResultUnlisten: (() => void) | null = null;
let writeChunkUnlisten: (() => void) | null = null;
let writeDoneUnlisten: (() => void) | null = null;

onMounted(async () => {
  loadSessions();
  scrollToBottom();

  // 监听 AI 生成的大纲结果（来自 function calling 的 save_outline 拦截）
  outlineResultUnlisten = await listen('ai-outline-result', (event) => {
    const data = event.payload as {
      sessionId: string;
      bookId: string;
      chapterId: string;
      outlineType: 'coarse' | 'fine';
      content: string;
    };
    // 只处理当前会话的结果（新会话时 currentSessionId 尚为 undefined，也接受）
    if (currentSessionId.value && data.sessionId !== currentSessionId.value) return;

    // 在聊天中插入大纲卡片消息
    const outlineMsg: AIChatMessage = {
      id: 'outline_' + Date.now(),
      bookId: data.bookId,
      sessionId: data.sessionId,
      role: 'outline' as any,
      content: JSON.stringify({
        chapterId: data.chapterId,
        outlineType: data.outlineType,
        content: data.content,
      }),
      timestamp: Date.now(),
    };
    messages.value.push(outlineMsg);
    scrollToBottom();
  });

  // 监听 AI 生成的创作结果（来自 function calling 的 write_chapter 拦截）
  // 写作AI流式生成 - 开始
  let currentWriteMsgId: string | null = null;
  writeResultUnlisten = await listen('ai-write-start', (event) => {
    const data = event.payload as { sessionId: string; bookId: string; chapterId: string; description: string };
    if (currentSessionId.value && data.sessionId !== currentSessionId.value) return;
    const writeMsg: AIChatMessage = {
      id: 'write_' + Date.now(),
      bookId: data.bookId,
      sessionId: data.sessionId,
      role: 'write' as any,
      content: JSON.stringify({ chapterId: data.chapterId, content: '', description: data.description, streaming: true }),
      timestamp: Date.now(),
    };
    currentWriteMsgId = writeMsg.id;
    writeInsertMode.value[writeMsg.id] = 'append';
    messages.value.push(writeMsg);
    scrollToBottom();
  });

  // 写作AI流式生成 - 内容块
  writeChunkUnlisten = await listen('ai-write-chunk', (event) => {
    const data = event.payload as { sessionId: string; chunk: string };
    if (!currentWriteMsgId) return;
    const msg = messages.value.find(m => m.id === currentWriteMsgId);
    if (msg) {
      const parsed = parseWriteMsg(msg);
      if (parsed) {
        parsed.content += data.chunk;
        msg.content = JSON.stringify(parsed);
      }
      scrollToBottom();
    }
  });

  // 写作AI流式生成 - 完成
  writeDoneUnlisten = await listen('ai-write-result', (event) => {
    const data = event.payload as { sessionId: string; bookId: string; chapterId: string; content: string; description: string };
    if (currentSessionId.value && data.sessionId !== currentSessionId.value) return;
    // 找到当前的流式卡片，更新为完成状态
    if (currentWriteMsgId) {
      const msg = messages.value.find(m => m.id === currentWriteMsgId);
      if (msg) {
        msg.content = JSON.stringify({ chapterId: data.chapterId, content: data.content, description: data.description });
      }
    } else {
      // 非流式到达（直接完成），创建新卡片
      const writeMsg: AIChatMessage = {
        id: 'write_' + Date.now(),
        bookId: data.bookId,
        sessionId: data.sessionId,
        role: 'write' as any,
        content: JSON.stringify({ chapterId: data.chapterId, content: data.content, description: data.description }),
        timestamp: Date.now(),
      };
      writeInsertMode.value[writeMsg.id] = 'append';
      messages.value.push(writeMsg);
    }
    currentWriteMsgId = null;
    scrollToBottom();
  });

  // 监听 AI 报告的设定冲突（来自 function calling 的 report_conflict）
  conflictResultUnlisten = await listen('ai-conflict-result', (event) => {
    const data = event.payload as {
      sessionId: string;
      bookId: string;
      conflictId: string;
      description: string;
      suggestion: string;
      severity: string;
    };
    if (data.sessionId !== currentSessionId.value) return;

    const conflictMsg: AIChatMessage = {
      id: 'conflict_' + Date.now(),
      bookId: data.bookId,
      sessionId: data.sessionId,
      role: 'conflict' as any,
      content: JSON.stringify({
        conflictId: data.conflictId,
        description: data.description,
        suggestion: data.suggestion,
        severity: data.severity,
      }),
      timestamp: Date.now(),
    };
    messages.value.push(conflictMsg);
    scrollToBottom();
  });
});

onUnmounted(() => {
  if (streamUnlisten.value) {
    streamUnlisten.value();
  }
  if (outlineResultUnlisten) {
    outlineResultUnlisten();
  }
  if (writeResultUnlisten) {
    writeResultUnlisten();
  }
  if (conflictResultUnlisten) {
    conflictResultUnlisten();
  }
  if (writeChunkUnlisten) {
    writeChunkUnlisten();
  }
  if (writeDoneUnlisten) {
    writeDoneUnlisten();
  }
  polishUnwatch();
});

</script>

<template>
  <div class="ai-chat-panel">
    <!-- 头部 -->
    <header class="chat-header">
      <div class="header-brand">
        <div class="brand-icon">
          <el-icon :size="20"><Cpu /></el-icon>
        </div>
        <span class="brand-title">AI 助手</span>
      </div>
      <div class="header-actions">
        <el-button
          text
          :icon="Plus"
          @click="createNewSession"
          title="新建会话"
          class="header-btn"
        />
        <el-button
          text
          :icon="ArrowDown"
          @click="showSessionList = !showSessionList"
          title="历史会话"
          class="header-btn"
        />
        <el-button
          text
          :icon="Close"
          @click="emit('close')"
          title="关闭"
          class="header-btn"
        />
      </div>
    </header>

    <!-- 会话列表面板 -->
    <el-drawer
      v-model="showSessionList"
      title="历史会话"
      direction="ltr"
      size="300px"
      class="session-drawer"
    >
      <div class="session-list">
        <div
          v-for="session in sessions"
          :key="session.sessionId"
          :class="['session-item', { active: session.sessionId === currentSessionId }]"
          @click="switchSession(session)"
        >
          <div class="session-info">
            <div class="session-title">{{ session.title }}</div>
            <div class="session-meta">
              {{ new Date(session.updatedAt).toLocaleDateString() }}
              · {{ session.messageCount }} 条消息
            </div>
          </div>
          <el-button
            link
            type="danger"
            :icon="Delete"
            @click="handleDeleteSession(session, $event)"
            class="delete-btn"
          />
        </div>
        <div v-if="sessions.length === 0" class="session-empty">
          <el-empty description="暂无历史会话" />
        </div>
      </div>
    </el-drawer>

    <!-- 消息列表 -->
    <div ref="messagesContainer" class="messages-container">
      <div v-if="messages.length === 0" class="welcome-state">
        <div class="welcome-icon">
          <el-icon :size="48"><Cpu /></el-icon>
        </div>
        <h3 class="welcome-title">AI 写作助手</h3>
        <p class="welcome-desc">我可以帮你：</p>
        <ul class="welcome-features">
          <li>分析小说剧情和角色</li>
          <li>提供写作建议和润色</li>
          <li>回答关于章节内容的问题</li>
          <li>协助构思情节发展</li>
        </ul>
      </div>
      
      <template v-else>
        <template v-for="msg in messages" :key="msg.id">
          <!-- 工具调用消息 -->
          <div v-if="msg.role === 'tool'" class="tool-call-wrapper">
            <div
              v-for="(item, index) in parseToolItems(msg.content)"
              :key="index"
              class="tool-call-item"
            >
              {{ item }}
            </div>
          </div>
          <!-- 大纲结果消息（来自 function calling 的 save_outline 拦截） -->
          <div v-else-if="msg.role === 'outline' && parseOutlineMsg(msg)" class="outline-result-card">
            <div class="outline-type-header">
              {{ parseOutlineMsg(msg)!.outlineType === 'coarse' ? '粗纲' : '细纲' }}
            </div>
            <div class="message-text markdown-body" v-html="renderMarkdown(parseOutlineMsg(msg)!.content)"></div>
            <div v-if="!historyMessageIds.has(msg.id)" class="outline-actions-wrapper">
              <el-button
                v-if="!isOutlineSaved(msg.id, parseOutlineMsg(msg)!.outlineType)"
                type="primary"
                size="small"
                :icon="Check"
                @click="applyOutline(msg, parseOutlineMsg(msg)!.chapterId, parseOutlineMsg(msg)!.outlineType, parseOutlineMsg(msg)!.content)"
              >
                保存{{ parseOutlineMsg(msg)!.outlineType === 'coarse' ? '粗纲' : '细纲' }}
              </el-button>
              <el-button
                size="small"
                :icon="CloseIcon"
                @click="cancelOutline(msg)"
                class="cancel-btn"
              >
                取消
              </el-button>
            </div>
          </div>
          <!-- 创作结果消息（来自 function calling 的 write_chapter 拦截） -->
          <div v-else-if="msg.role === 'write' && parseWriteMsg(msg)" class="write-result-card">
            <template v-if="!isWriteApplied(msg)">
              <div class="write-card-header">
                {{ (parseWriteMsg(msg) as any).streaming ? '正在生成...' : '创作结果' }}{{ parseWriteMsg(msg)!.description ? '：' + parseWriteMsg(msg)!.description : '' }}
              </div>
              <div class="message-text markdown-body" v-html="renderMarkdown(parseWriteMsg(msg)!.content || '（生成中...）')"></div>
              <div v-if="!historyMessageIds.has(msg.id) && !(parseWriteMsg(msg) as any).streaming" class="outline-actions-wrapper">
                <div style="display:flex;align-items:center;gap:8px;margin-bottom:4px;">
                  <span style="font-size:12px;color:var(--nw-text-secondary);">插入方式：</span>
                  <el-radio-group v-model="writeInsertMode[msg.id]" size="small">
                    <el-radio value="append">追加到末尾</el-radio>
                    <el-radio value="replace">替换全文</el-radio>
                  </el-radio-group>
                </div>
                <div style="display:flex;gap:8px;">
                  <el-button type="primary" size="small" :icon="Check" @click="applyWrite(msg)">
                    {{ writeInsertMode[msg.id] === 'replace' ? '替换全文' : '追加到末尾' }}
                  </el-button>
                  <el-button size="small" :icon="CloseIcon" @click="cancelWrite(msg)" class="cancel-btn">取消</el-button>
                </div>
              </div>
            </template>
            <template v-else>
              <div class="write-card-header">已应用</div>
              <div class="write-applied-text">{{ parseWriteMsg(msg)!.description || '创作内容' }} — 已保存到章节</div>
            </template>
          </div>
          <!-- 设定冲突消息（来自 function calling 的 report_conflict） -->
          <div v-else-if="msg.role === 'conflict'" class="conflict-card">
            <div class="conflict-card-header">
              <el-tag :type="(JSON.parse(msg.content).severity === 'high' ? 'danger' : JSON.parse(msg.content).severity === 'medium' ? 'warning' : 'info')" size="small">
                {{ JSON.parse(msg.content).severity === 'high' ? '严重冲突' : JSON.parse(msg.content).severity === 'medium' ? '中等冲突' : '轻微冲突' }}
              </el-tag>
            </div>
            <div class="conflict-card-desc">{{ JSON.parse(msg.content).description }}</div>
            <div v-if="JSON.parse(msg.content).suggestion" class="conflict-card-suggestion">
              建议：{{ JSON.parse(msg.content).suggestion }}
            </div>
          </div>
          <!-- 普通消息 -->
          <div v-else :class="['message', msg.role]">
            <div class="message-avatar">
              <el-avatar
                v-if="msg.role === 'assistant'"
                :icon="Cpu"
                :size="36"
                class="ai-avatar"
              />
              <el-avatar
                v-else
                :icon="UserFilled"
                :size="36"
                class="user-avatar"
              />
            </div>
            <div class="message-bubble">
              <!-- 润色结果显示 -->
              <template v-if="msg.role === 'assistant' && parsePolishResult(msg.content).hasResult">
                <div class="polish-result">
                  <div class="polish-label">润色结果</div>
                  <div class="polish-text">{{ parsePolishResult(msg.content).polishedText }}</div>
                </div>
                <div v-if="!msg.polishHandled && !historyMessageIds.has(msg.id)" class="polish-actions">
                  <el-button
                    type="primary"
                    size="small"
                    :icon="Check"
                    @click="applyPolish(msg, parsePolishResult(msg.content).originalText!, parsePolishResult(msg.content).polishedText!)"
                  >
                    应用
                  </el-button>
                  <el-button
                    type="info"
                    size="small"
                    :icon="CloseIcon"
                    @click="cancelPolish(msg)"
                  >
                    取消
                  </el-button>
                </div>
              </template>
              <!-- 普通消息显示 -->
              <div v-else class="message-text markdown-body" v-html="renderMarkdown(getDisplayContent(msg.content))"></div>
            </div>
          </div>
        </template>
      </template>
      
    </div>

    <!-- 输入区域 -->
    <footer class="input-area">
      <div class="input-wrapper">
        <el-input
          v-model="inputMessage"
          type="textarea"
          :rows="3"
          placeholder="输入消息，AI 会自动查询相关章节内容来回答你..."
          :disabled="isLoading"
          @keydown.enter.prevent="handleSend"
          class="chat-input"
        />
        <el-button
          type="primary"
          :icon="Promotion"
          :loading="isLoading"
          @click="handleSend"
          class="send-btn"
          circle
        />
      </div>
      <div class="input-footer">
        <div class="session-status">
          <span v-if="isStreaming" class="status-streaming">
            <span class="status-dot"></span>
            {{ currentAgentPhase === 'intent' ? '分析意图...' :
               currentAgentPhase === 'tool' ? '查询数据...' :
               currentAgentPhase === 'writing' ? '创作中...' :
               currentAgentPhase === 'outlining' ? '生成大纲...' :
               currentAgentPhase === 'polishing' ? '润色中...' :
               'AI 正在思考...' }}
          </span>
          <span v-else-if="currentSessionId" class="status-session">
            当前会话: {{ currentSessionId.slice(0, 8) }}...
          </span>
          <span v-else class="status-new">新会话</span>
        </div>
        <span class="input-hint">按 Enter 发送</span>
      </div>
    </footer>
  </div>
</template>

<style scoped>
.ai-chat-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--nw-bg-primary);
}

/* 头部 */
.chat-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--nw-space-md);
  border-bottom: 1px solid var(--nw-border-light);
  background: var(--nw-bg-secondary);
  flex-shrink: 0;
}

.header-brand {
  display: flex;
  align-items: center;
  gap: 10px;
}

.brand-icon {
  width: 36px;
  height: 36px;
  border-radius: var(--nw-radius);
  background: linear-gradient(135deg, var(--nw-primary) 0%, var(--nw-primary-light) 100%);
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
}

.brand-title {
  font-weight: 600;
  font-size: 16px;
  color: var(--nw-text-primary);
}

.header-actions {
  display: flex;
  gap: 4px;
}

.header-btn {
  color: var(--nw-text-secondary);
}

.header-btn:hover {
  color: var(--nw-primary);
}

/* 会话抽屉 */
.session-drawer :deep(.el-drawer__header) {
  margin-bottom: 0;
  padding: var(--nw-space-md) var(--nw-space-lg);
  border-bottom: 1px solid var(--nw-border-light);
}

.session-drawer :deep(.el-drawer__body) {
  padding: var(--nw-space-sm);
  background: var(--nw-bg-secondary);
}

.session-list {
  overflow-y: auto;
}

.session-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--nw-space-md);
  border-radius: var(--nw-radius-sm);
  cursor: pointer;
  transition: all var(--nw-transition-fast);
  margin-bottom: var(--nw-space-xs);
  background: var(--nw-bg-primary);
}

.session-item:hover {
  background: var(--nw-bg-hover);
  transform: translateX(2px);
}

.session-item.active {
  background: var(--nw-bg-tertiary);
  border-left: 3px solid var(--nw-primary);
}

.session-info {
  flex: 1;
  min-width: 0;
}

.session-title {
  font-size: 14px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--nw-text-primary);
}

.session-meta {
  font-size: 12px;
  color: var(--nw-text-tertiary);
  margin-top: 4px;
  font-family: var(--nw-font-mono);
}

.delete-btn {
  opacity: 0;
  transition: opacity var(--nw-transition-fast);
}

.session-item:hover .delete-btn {
  opacity: 1;
}

.session-empty {
  padding: var(--nw-space-xl);
}

/* 消息列表 */
.messages-container {
  flex: 1;
  overflow-y: auto;
  padding: var(--nw-space-lg);
  display: flex;
  flex-direction: column;
  gap: var(--nw-space-md);
  background: var(--nw-bg-page);
}

/* 欢迎状态 */
.welcome-state {
  text-align: center;
  padding: var(--nw-space-xl) var(--nw-space-lg);
  color: var(--nw-text-secondary);
  margin: auto 0;
}

.welcome-icon {
  width: 80px;
  height: 80px;
  border-radius: 50%;
  background: linear-gradient(135deg, var(--nw-primary-light) 0%, var(--nw-primary) 100%);
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0 auto var(--nw-space-lg);
  color: white;
  box-shadow: var(--nw-shadow);
}

.welcome-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--nw-text-primary);
  margin: 0 0 var(--nw-space) 0;
  font-family: var(--nw-font-display);
}

.welcome-desc {
  font-size: 14px;
  color: var(--nw-text-tertiary);
  margin-bottom: var(--nw-space);
}

.welcome-features {
  list-style: none;
  padding: 0;
  margin: 0;
  display: inline-block;
  text-align: left;
}

.welcome-features li {
  font-size: 14px;
  padding: 6px 0;
  padding-left: 24px;
  position: relative;
  color: var(--nw-text-secondary);
}

.welcome-features li::before {
  content: '✓';
  position: absolute;
  left: 0;
  color: var(--nw-success);
  font-weight: 600;
}

/* 消息气泡 */
.message {
  display: flex;
  gap: var(--nw-space-sm);
  max-width: 100%;
  animation: messageAppear 0.3s ease-out;
}

@keyframes messageAppear {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.message.user {
  flex-direction: row-reverse;
}

.message-avatar {
  flex-shrink: 0;
}

.ai-avatar {
  background: linear-gradient(135deg, var(--nw-primary) 0%, var(--nw-primary-light) 100%);
  color: white;
}

.user-avatar {
  background: var(--nw-accent);
  color: white;
}

.message-bubble {
  max-width: calc(100% - 60px);
  padding: var(--nw-space) var(--nw-space-md);
  border-radius: var(--nw-radius);
  background: var(--nw-bg-primary);
  word-wrap: break-word;
  line-height: 1.7;
  align-self: flex-start;
  box-shadow: var(--nw-shadow-sm);
  border: 1px solid var(--nw-border-light);
}

.message.user .message-bubble {
  background: var(--nw-primary);
  color: white;
  border-color: var(--nw-primary);
}

.message-text {
  white-space: normal;
}

/* 润色结果显示 */
.polish-result {
  margin-bottom: var(--nw-space-md);
}

.polish-label {
  font-size: 12px;
  color: var(--nw-text-secondary);
  margin-bottom: var(--nw-space-sm);
  font-weight: 500;
}

.polish-text {
  font-size: 14px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-wrap: break-word;
}

/* 润色操作按钮 */
.polish-actions {
  display: flex;
  gap: var(--nw-space);
  margin-top: var(--nw-space-md);
  padding-top: var(--nw-space-md);
  border-top: 1px solid var(--nw-border-light);
}

.polish-actions .el-button {
  flex: 1;
  justify-content: center;
  padding: var(--nw-space-sm) var(--nw-space);
}

/* 大纲结果卡片（function calling 拦截的 save_outline） */
.outline-result-card {
  display: flex;
  flex-direction: column;
  margin: var(--nw-space-sm) 0;
  padding: var(--nw-space);
  background: var(--nw-bg-secondary);
  border-radius: var(--nw-radius-md);
  border: 1px solid var(--nw-border);
  border-left: 3px solid var(--nw-primary);
}

.outline-result-card .outline-type-header {
  font-size: 13px;
  font-weight: 600;
  color: var(--nw-primary);
  margin-bottom: var(--nw-space-sm);
}

.outline-result-card .message-text {
  font-size: 14px;
  line-height: 1.6;
}

.outline-result-card .outline-actions-wrapper {
  margin-top: var(--nw-space);
  padding-top: var(--nw-space);
  border-top: 1px solid var(--nw-border-light);
  display: flex;
  gap: var(--nw-space-sm);
}

/* 创作结果卡片 */
.write-result-card {
  display: flex;
  flex-direction: column;
  margin: var(--nw-space-sm) 0;
  padding: var(--nw-space);
  background: var(--nw-bg-secondary);
  border-radius: var(--nw-radius-md);
  border: 1px solid var(--nw-border);
  border-left: 3px solid var(--el-color-success);
}
.write-card-header {
  font-size: 13px;
  font-weight: 600;
  color: var(--el-color-success);
  margin-bottom: var(--nw-space-sm);
}
.write-applied-text {
  font-size: 13px;
  color: var(--nw-text-secondary);
}

/* Markdown 样式 */
.markdown-body :deep(h1),
.markdown-body :deep(h2),
.markdown-body :deep(h3),
.markdown-body :deep(h4) {
  margin: 12px 0 8px;
  font-weight: 600;
}

.markdown-body :deep(h1) { font-size: 1.4em; }
.markdown-body :deep(h2) { font-size: 1.2em; }
.markdown-body :deep(h3) { font-size: 1.1em; }

.markdown-body :deep(p) {
  margin: 8px 0;
}

.markdown-body :deep(ul),
.markdown-body :deep(ol) {
  margin: 8px 0;
  padding-left: 20px;
}

.markdown-body :deep(li) {
  margin: 4px 0;
}

.markdown-body :deep(code) {
  background: var(--nw-bg-secondary);
  padding: 2px 6px;
  border-radius: var(--nw-radius-sm);
  font-family: var(--nw-font-mono);
  font-size: 0.9em;
}

.markdown-body :deep(pre) {
  background: var(--nw-bg-tertiary);
  padding: 12px;
  border-radius: var(--nw-radius-sm);
  overflow-x: auto;
  margin: 8px 0;
}

.markdown-body :deep(pre code) {
  background: transparent;
  padding: 0;
}

.markdown-body :deep(blockquote) {
  border-left: 3px solid var(--nw-accent);
  margin: 8px 0;
  padding-left: 12px;
  color: var(--nw-text-secondary);
}

.markdown-body :deep(hr) {
  border: none;
  border-top: 1px solid var(--nw-border);
  margin: 12px 0;
}

.markdown-body :deep(table) {
  border-collapse: collapse;
  width: 100%;
  margin: 8px 0;
}

.markdown-body :deep(th),
.markdown-body :deep(td) {
  border: 1px solid var(--nw-border);
  padding: 8px;
  text-align: left;
}

.markdown-body :deep(th) {
  background: var(--nw-bg-secondary);
}

/* 用户消息的 Markdown 样式调整 */
.message.user .markdown-body :deep(code) {
  background: rgba(255, 255, 255, 0.2);
}

.message.user .markdown-body :deep(pre) {
  background: rgba(0, 0, 0, 0.2);
}

.message.user .markdown-body :deep(blockquote) {
  border-left-color: rgba(255, 255, 255, 0.5);
  color: rgba(255, 255, 255, 0.9);
}

/* 输入区域 */
.input-area {
  padding: var(--nw-space-md);
  border-top: 1px solid var(--nw-border-light);
  background: var(--nw-bg-secondary);
  flex-shrink: 0;
}

.input-wrapper {
  display: flex;
  gap: var(--nw-space-sm);
  align-items: flex-end;
}

.chat-input :deep(.el-textarea__inner) {
  background: var(--nw-bg-primary);
  border-color: var(--nw-border);
  border-radius: var(--nw-radius);
  resize: none;
  padding: var(--nw-space);
  font-size: 14px;
}

.chat-input :deep(.el-textarea__inner:focus) {
  border-color: var(--nw-primary);
}

.send-btn {
  flex-shrink: 0;
  margin-bottom: 4px;
}

.input-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: var(--nw-space-sm);
  font-size: 12px;
}

.session-status {
  color: var(--nw-text-tertiary);
}

.status-streaming {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--nw-primary);
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--nw-primary);
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.status-session {
  font-family: var(--nw-font-mono);
}

.status-new {
  color: var(--nw-accent);
}

.input-hint {
  color: var(--nw-text-muted);
}

/* 滚动条 */
.messages-container::-webkit-scrollbar,
.session-list::-webkit-scrollbar {
  width: 4px;
}

.messages-container::-webkit-scrollbar-track,
.session-list::-webkit-scrollbar-track {
  background: transparent;
}

.messages-container::-webkit-scrollbar-thumb,
.session-list::-webkit-scrollbar-thumb {
  background: var(--nw-border);
  border-radius: 2px;
}

.messages-container::-webkit-scrollbar-thumb:hover,
.session-list::-webkit-scrollbar-thumb:hover {
  background: var(--nw-text-muted);
}

/* 工具调用消息样式 - 竖排简洁版 */
.tool-call-wrapper {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin: 2px 0;
  gap: 2px;
}

.tool-call-item {
  font-size: 12px;
  color: var(--nw-text-muted);
  padding: 1px 0;
}

/* 设定冲突卡片 */
.conflict-card {
  padding: 10px 12px;
  margin: 4px 0;
  background: var(--nw-bg-secondary);
  border-radius: 8px;
  border-left: 3px solid var(--el-color-warning);
}
.conflict-card-header { margin-bottom: 6px; }
.conflict-card-desc { font-size: 13px; line-height: 1.5; }
.conflict-card-suggestion { font-size: 12px; color: var(--nw-text-secondary); margin-top: 4px; }

</style>
