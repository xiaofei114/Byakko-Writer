import { ref } from 'vue';

// 润色请求事件
export interface PolishRequest {
  text: string;
  chapterId?: string;
}

// 创建简单的事件总线
const polishRequest = ref<PolishRequest | null>(null);

export function requestPolish(text: string, chapterId?: string) {
  polishRequest.value = { text, chapterId };
}

export function getPolishRequest() {
  return polishRequest;
}

export function clearPolishRequest() {
  polishRequest.value = null;
}
