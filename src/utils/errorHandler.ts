import { ElMessage, ElNotification } from 'element-plus';

export interface ParsedError {
  kind: 'network' | 'api' | 'auth' | 'model' | 'timeout' | 'config' | 'unknown';
  message: string;
  detail?: string;
}

const HTTP_STATUS_MAP: Record<number, string> = {
  400: '请求参数错误',
  401: 'API Key 无效或已过期',
  403: '无权访问该接口',
  404: '接口不存在',
  429: '请求过于频繁，请稍后重试',
  500: 'AI 服务内部错误',
  502: 'AI 服务网关错误',
  503: 'AI 服务暂时不可用',
  504: 'AI 服务响应超时',
};

const API_ERROR_KEYWORDS = [
  { keyword: 'insufficient_quota', kind: 'auth' as const, message: 'API 额度不足' },
  { keyword: 'invalid_api_key', kind: 'auth' as const, message: 'API Key 无效' },
  { keyword: 'rate_limit', kind: 'timeout' as const, message: '请求频率超限，请稍后重试' },
  { keyword: 'context_length', kind: 'api' as const, message: '上下文长度超出限制' },
  { keyword: 'model_not_found', kind: 'model' as const, message: '模型不存在或不可用' },
  { keyword: 'billing', kind: 'auth' as const, message: '账户余额不足' },
];

export function parseError(error: unknown): ParsedError {
  const errorStr = String(error);
  const lower = errorStr.toLowerCase();

  // 网络错误
  if (lower.includes('network error') || lower.includes('fetch failed') || lower.includes('connection')) {
    return {
      kind: 'network',
      message: '网络连接失败',
      detail: '请检查网络连接和 API 地址是否正确',
    };
  }

  // 超时
  if (lower.includes('timeout') || lower.includes('timed out')) {
    return {
      kind: 'timeout',
      message: '请求超时',
      detail: 'AI 服务响应时间过长，请检查网络或稍后重试',
    };
  }

  // API Key 未配置
  if (lower.includes('api key') && (lower.includes('未配置') || lower.includes('empty') || lower.includes('not set'))) {
    return {
      kind: 'config',
      message: 'API Key 未配置',
      detail: '请在设置中配置 AI API Key',
    };
  }

  // HTTP 状态码错误
  const statusMatch = errorStr.match(/(\d{3})/);
  if (statusMatch) {
    const status = parseInt(statusMatch[1], 10);
    const statusMessage = HTTP_STATUS_MAP[status];
    
    if (statusMessage) {
      // 提取错误详情
      const detailMatch = errorStr.match(/:\s*(.+)/);
      let detail = '';
      if (detailMatch) {
        try {
          const jsonStr = detailMatch[1];
          const parsed = JSON.parse(jsonStr);
          detail = parsed.error?.message || parsed.message || jsonStr.substring(0, 200);
        } catch {
          detail = detailMatch[1].substring(0, 200);
        }
      }
      
      return {
        kind: status >= 500 ? 'api' : status === 401 ? 'auth' : 'unknown',
        message: statusMessage,
        detail: detail || undefined,
      };
    }
  }

  // API 返回的错误关键词
  for (const { keyword, kind, message } of API_ERROR_KEYWORDS) {
    if (lower.includes(keyword)) {
      return { kind, message, detail: errorStr.substring(0, 200) };
    }
  }

  // API 错误
  if (lower.includes('api错误') || lower.includes('api error') || lower.includes('api:')) {
    const detailMatch = errorStr.match(/:\s*(.+)/);
    const detail = detailMatch ? detailMatch[1].substring(0, 200) : undefined;
    return {
      kind: 'api',
      message: 'AI 接口返回错误',
      detail,
    };
  }

  // 未知错误
  return {
    kind: 'unknown',
    message: '请求失败',
    detail: errorStr.substring(0, 200),
  };
}

export function getErrorIcon(kind: ParsedError['kind']): string {
  switch (kind) {
    case 'network': return '🌐';
    case 'api': return '⚠️';
    case 'auth': return '🔑';
    case 'model': return '🤖';
    case 'timeout': return '⏱️';
    case 'config': return '⚙️';
    default: return '❌';
  }
}

export function showError(error: unknown, options?: { title?: string; duration?: number }) {
  const parsed = parseError(error);
  const icon = getErrorIcon(parsed.kind);
  const title = options?.title || `${icon} ${parsed.message}`;
  const message = parsed.detail ? `${parsed.message}：${parsed.detail}` : parsed.message;

  ElNotification({
    title,
    message,
    type: 'error',
    duration: options?.duration ?? 5000,
    dangerouslyUseHTMLString: false,
  });
}

export function showSimpleError(error: unknown) {
  const parsed = parseError(error);
  const msg = parsed.detail ? `${parsed.message}：${parsed.detail}` : parsed.message;
  ElMessage.error(msg);
}
