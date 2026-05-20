<script setup lang="ts">
import { ref, nextTick, onMounted, onUnmounted, watch } from 'vue';
import { useBookStore } from '../stores/book';
import { useConfigStore } from '../stores/config';
import { sendChatMessageStream, getChatSessions, deleteChatSession, getChatHistory } from '../api/aiChat';
import type { AIChatMessage, ChatSession } from '../types';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { listen } from '@tauri-apps/api/event';
import { marked } from 'marked';
import { Delete, Close, UserFilled, Cpu, Promotion, Plus, ArrowDown, Check, Close as CloseIcon } from '@element-plus/icons-vue';
import { ElMessage, ElMessageBox } from 'element-plus';
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

// 过滤工具调用标记和润色提示词
const filterToolCalls = (content: string): string => {
  // 先保存 outline_result 和 polish_result 内容
  const outlineResults: string[] = [];
  const polishResults: string[] = [];
  
  let temp = content.replace(/<outline_result>[\s\S]*?<\/outline_result>/g, (match) => {
    outlineResults.push(match);
    return `___OUTLINE_RESULT_${outlineResults.length - 1}___`;
  });
  
  temp = temp.replace(/<polish_result>[\s\S]*?<\/polish_result>/g, (match) => {
    polishResults.push(match);
    return `___POLISH_RESULT_${polishResults.length - 1}___`;
  });
  
  // 移除 XML 格式的工具调用（包括内容）
  let cleaned = temp.replace(/<tool[^>]*>[\s\S]*?<\/tool>/g, '');
  // 移除 Markdown 代码块格式的工具调用
  cleaned = cleaned.replace(/```tool:[\s\S]*?```/g, '');
  // 移除 JSON 格式的工具调用
  cleaned = cleaned.replace(/\{\s*"tool"\s*:[\s\S]*?\}\s*/g, '');
  // 移除工具参数 JSON（如 { "bookId": "...", "chapterId": "..." }）
  // 匹配包含 bookId、chapterId、outlineType 等工具参数的 JSON
  cleaned = cleaned.replace(/\{\s*"bookId"\s*:\s*"[^"]+"\s*,\s*"chapterId"\s*:\s*"[^"]+"[\s\S]*?\}\s*/g, '');
  // 移除单独的参数行（如 "bookId": "..."）
  cleaned = cleaned.replace(/"bookId"\s*:\s*"[^"]+",?\s*/g, '');
  cleaned = cleaned.replace(/"chapterId"\s*:\s*"[^"]+",?\s*/g, '');
  cleaned = cleaned.replace(/"outlineType"\s*:\s*"[^"]+",?\s*/g, '');
  cleaned = cleaned.replace(/"content"\s*:\s*"[\s\S]*?"(,?\s*)?/g, '');
  // 移除残留的 JSON 符号
  cleaned = cleaned.replace(/[{}"]/g, '');
  // 移除润色提示词中的格式说明（从"请直接返回"到格式说明结束）
  cleaned = cleaned.replace(/请直接返回润色后的文本[\s\S]*$/g, '');
  cleaned = cleaned.replace(/必须在回复末尾使用以下格式[\s\S]*$/g, '');
  
  // 恢复 outline_result 和 polish_result
  outlineResults.forEach((match, i) => {
    cleaned = cleaned.replace(`___OUTLINE_RESULT_${i}___`, match);
  });
  polishResults.forEach((match, i) => {
    cleaned = cleaned.replace(`___POLISH_RESULT_${i}___`, match);
  });
  
  return cleaned.trim();
};

// 解析润色结果
const parsePolishResult = (content: string): { hasResult: boolean; displayContent: string; originalText?: string; polishedText?: string } => {
  // 匹配新的格式：<polish_result>润色后：xxx</polish_result>
  const match = content.match(/<polish_result>\s*润色后：\s*([\s\S]*?)\s*<\/polish_result>/);
  if (match) {
    const polishedText = match[1].trim();
    // 移除润色结果标记，显示简洁版本
    const displayContent = content.replace(/<polish_result>[\s\S]*?<\/polish_result>/g, '').trim();
    // 使用保存的原文
    return { hasResult: true, displayContent, originalText: currentPolishOriginalText, polishedText };
  }
  return { hasResult: false, displayContent: content };
};

// 大纲结果类型
interface OutlineResult {
  outlineType: string;
  chapterId: string;
  outlineContent: string;
}

// 解析所有大纲结果（支持同时返回粗纲和细纲）
const parseAllOutlineResults = (content: string): { hasResult: boolean; displayContent: string; outlines: OutlineResult[] } => {
  // 如果没有内容，返回空结果
  if (!content) {
    return { hasResult: false, displayContent: '', outlines: [] };
  }
  
  // 处理可能的 HTML 转义
  let normalizedContent = content
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&');
  
  // 查找所有 <outline_result> 块
  const outlineRegex = /<outline_result>([\s\S]*?)<\/outline_result>/g;
  const outlines: OutlineResult[] = [];
  let match;
  
  while ((match = outlineRegex.exec(normalizedContent)) !== null) {
    const innerContent = match[1];
    
    // 尝试提取章节ID
    const idMatch = innerContent.match(/章节ID[：:\s]+([a-zA-Z0-9_]+)/);
    const chapterId = idMatch ? idMatch[1].trim() : '';
    
    // 尝试提取大纲类型
    const typeMatch = innerContent.match(/大纲类型[：:\s]+(\w+)/);
    const outlineType = typeMatch ? typeMatch[1].trim() : 'fine';
    
    // 尝试提取大纲内容（更精确地匹配"大纲内容："后面的内容）
    const contentMatch = innerContent.match(/大纲内容[：:]\s*\n?([\s\S]*?)(?=\n<\/outline_result>|$)/i);
    let outlineContent = contentMatch ? contentMatch[1].trim() : innerContent.trim();
    
    // 清理可能包含的元数据行
    outlineContent = outlineContent
      .replace(/^大纲类型[：:]\s*\w+\s*$/gim, '')
      .replace(/^章节ID[：:]\s*[a-zA-Z0-9_]+\s*$/gim, '')
      .replace(/^大纲[：:]?\s*$/gim, '')
      .trim();
    
    if (chapterId) {
      outlines.push({ outlineType, chapterId, outlineContent });
    }
  }
  
  // 移除所有大纲结果标记，显示简洁版本
  const displayContent = normalizedContent.replace(/<outline_result>[\s\S]*?<\/outline_result>/g, '').trim();
  
  return { 
    hasResult: outlines.length > 0, 
    displayContent, 
    outlines 
  };
};

// 兼容旧版本的单个大纲解析
const parseOutlineResult = (content: string): { hasResult: boolean; displayContent: string; outlineType?: string; chapterId?: string; outlineContent?: string } => {
  const result = parseAllOutlineResults(content);
  if (result.hasResult && result.outlines.length > 0) {
    const first = result.outlines[0];
    return {
      hasResult: true,
      displayContent: result.displayContent,
      outlineType: first.outlineType,
      chapterId: first.chapterId,
      outlineContent: first.outlineContent
    };
  }
  return { hasResult: false, displayContent: content };
};

// 获取显示内容（处理润色和大纲两种格式）
const getDisplayContent = (content: string): string => {
  // 先尝试解析润色结果
  const polishResult = parsePolishResult(content);
  if (polishResult.hasResult) {
    return polishResult.displayContent;
  }
  // 再尝试解析大纲结果
  const outlineResult = parseOutlineResult(content);
  if (outlineResult.hasResult) {
    return outlineResult.displayContent;
  }
  // 都不是，返回原始内容
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
const messagesContainer = ref<HTMLDivElement>();
const currentSessionId = ref<string | undefined>(undefined);
const sessions = ref<ChatSession[]>([]);
const showSessionList = ref(false);
const messages = ref<AIChatMessage[]>([]);
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

// 添加内容到打字机缓冲区
const addToTypewriterBuffer = (content: string) => {
  typewriterBuffer.value += content;
  startTypewriter(false); // 正常速度
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
    await scrollToBottom();
  } catch (error) {
    console.error('加载会话消息失败:', error);
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

  // 准备接收 AI 回复
  isLoading.value = true;
  isStreaming.value = true;
  currentStreamContent.value = '';

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
          
          // 构建带参数的显示文本
          let displayText = displayName;
          if (toolParams && Object.keys(toolParams).length > 0) {
            const paramTexts = Object.entries(toolParams)
              .map(([key, value]) => `${key}: ${value}`)
              .join(', ');
            displayText = `${displayName} (${paramTexts})`;
          }
          
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
        onComplete: (sessionId?: string) => {
          if (sessionId) {
            currentSessionId.value = sessionId;
          }
          isLoading.value = false;
          isStreaming.value = false;
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
          ElMessage.error('AI 请求失败: ' + error);
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
const handlePolishRequest = (request: import('../utils/eventBus').PolishRequest) => {
  // 保存原文供后续使用
  currentPolishOriginalText = request.text;

  // 构建润色提示词
  const polishPrompt = `请帮我润色以下文本，使其语言更加流畅自然、描写更加生动形象、节奏更加紧凑有力，但保持原有的情节和人物设定不变：

"""
${request.text}
"""

请直接返回润色后的文本，不要添加解释。必须在回复末尾使用以下格式：

<polish_result>
润色后：[润色后的完整文本]
</polish_result>`;

  // 自动发送消息
  inputMessage.value = polishPrompt;
  handleSend();
};

// 应用润色结果
const applyPolish = (msg: AIChatMessage, originalText: string, polishedText: string) => {
  // 标记已处理
  msg.polishHandled = true;

  // 发送事件到 WritingArea
  window.dispatchEvent(new CustomEvent('apply-polish', {
    detail: { originalText, polishedText }
  }));
  ElMessage.success('已应用润色结果');
};

// 取消润色
const cancelPolish = (msg: AIChatMessage) => {
  // 标记已处理
  msg.polishHandled = true;
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

// 取消大纲（取消所有未保存的大纲）
const cancelOutline = (msg: AIChatMessage) => {
  // 获取该消息的所有大纲
  const result = parseAllOutlineResults(msg.content);
  // 标记所有大纲为已处理（已取消）
  result.outlines.forEach(outline => {
    markOutlineSaved(msg.id, outline.outlineType);
  });
  ElMessage.info('已取消保存大纲');
};

// 组件卸载时清理
onUnmounted(() => {
  if (streamUnlisten.value) {
    streamUnlisten.value();
  }
  polishUnwatch();
});

onMounted(() => {
  loadSessions();
  scrollToBottom();
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
              {{ new Date(session.updatedAt * 1000).toLocaleDateString() }}
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
              v-for="(toolName, index) in JSON.parse(msg.content)" 
              :key="index"
              class="tool-call-item"
            >
              {{ toolName }}
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
              <div class="message-text markdown-body" v-html="renderMarkdown(getDisplayContent(msg.content))"></div>
              <!-- 润色结果操作按钮 -->
              <template v-if="msg.role === 'assistant' && !msg.polishHandled">
                <div v-if="parsePolishResult(msg.content).hasResult" class="polish-actions">
                  <el-button
                    type="primary"
                    size="small"
                    :icon="Check"
                    @click="applyPolish(msg, currentPolishOriginalText, parsePolishResult(msg.content).polishedText!)"
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
              <!-- 大纲结果操作按钮（支持多个大纲） -->
              <template v-if="msg.role === 'assistant' && !isStreaming">
                <div v-if="parseAllOutlineResults(msg.content).hasResult" class="outline-actions-wrapper">
                  <div 
                    v-for="(outline, index) in parseAllOutlineResults(msg.content).outlines.filter(o => !isOutlineSaved(msg.id, o.outlineType))" 
                    :key="index"
                    class="outline-action-item"
                  >
                    <span class="outline-type-label">{{ outline.outlineType === 'coarse' ? '粗纲' : '细纲' }}</span>
                    <el-button
                      type="primary"
                      size="small"
                      :icon="Check"
                      @click="applyOutline(msg, outline.chapterId, outline.outlineType, outline.outlineContent)"
                    >
                      保存{{ outline.outlineType === 'coarse' ? '粗纲' : '细纲' }}
                    </el-button>
                  </div>
                  <el-button
                    v-if="parseAllOutlineResults(msg.content).outlines.some(o => !isOutlineSaved(msg.id, o.outlineType))"
                    type="info"
                    size="small"
                    :icon="CloseIcon"
                    @click="cancelOutline(msg)"
                    class="cancel-btn"
                  >
                    全部取消
                  </el-button>
                </div>
              </template>
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
            AI 正在思考...
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

/* 大纲操作按钮样式 */
.outline-actions-wrapper {
  display: flex;
  flex-direction: column;
  gap: var(--nw-space-sm);
  margin-top: var(--nw-space-md);
  padding-top: var(--nw-space-md);
  border-top: 1px solid var(--nw-border-light);
}

.outline-action-item {
  display: flex;
  align-items: center;
  gap: var(--nw-space);
  padding: var(--nw-space-sm);
  background: var(--nw-bg-secondary);
  border-radius: var(--nw-radius-sm);
}

.outline-type-label {
  flex: 1;
  font-size: 14px;
  color: var(--nw-text-secondary);
  font-weight: 500;
}

.outline-actions-wrapper .cancel-btn {
  margin-top: var(--nw-space-sm);
  width: 100%;
  justify-content: center;
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
</style>
