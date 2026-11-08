// 统一的 Markdown 渲染入口
//
// marked 默认会把 Markdown 中的原始 HTML 原样输出，而 AI 生成的内容是
// 不可信输入（可能被提示词注入），所以渲染后必须经过白名单清洗再交给 v-html。

import { marked } from 'marked';
import { sanitizeHtml } from './sanitizeHtml';

marked.setOptions({
  breaks: true,
  gfm: true,
});

/**
 * 把 Markdown 渲染为可安全插入 DOM 的 HTML
 */
export function renderMarkdown(content: string | undefined | null): string {
  if (!content) return '';
  const html = marked.parse(content, { async: false });
  return sanitizeHtml(html);
}
