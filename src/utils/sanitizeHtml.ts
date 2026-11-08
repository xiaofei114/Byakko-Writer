// HTML 白名单清洗器
//
// AI 生成的 Markdown 经 marked 渲染后是通过 v-html 插入 DOM 的，
// marked 本身不做消毒，因此这里对结果做一层白名单过滤：
// - 危险标签（script/style/iframe/...）直接丢弃
// - 非白名单标签保留内容、去掉标签本身
// - 只保留白名单属性，移除所有 on* 事件属性
// - href/src 过滤 javascript:、vbscript:、file: 等协议

/** 允许保留的标签 */
const ALLOWED_TAGS = new Set([
  'a', 'b', 'blockquote', 'br', 'code', 'del', 'div', 'em', 'h1', 'h2', 'h3',
  'h4', 'h5', 'h6', 'hr', 'i', 'img', 'ins', 'li', 'ol', 'p', 'pre', 's',
  'span', 'strong', 'sub', 'sup', 'table', 'tbody', 'td', 'th', 'thead', 'tr', 'u', 'ul',
]);

/** 连同内容一起删除的危险标签 */
const DANGEROUS_TAGS = new Set([
  'script', 'style', 'iframe', 'object', 'embed', 'form', 'input', 'button',
  'textarea', 'select', 'option', 'link', 'meta', 'base', 'svg', 'math',
  'video', 'audio', 'source', 'template', 'noscript', 'frameset', 'frame',
]);

/** 允许保留的属性 */
const ALLOWED_ATTRS = new Set(['href', 'title', 'alt', 'src', 'class']);

function isSafeUrl(rawUrl: string, allowDataImage: boolean): boolean {
  const url = rawUrl.trim().toLowerCase();
  if (!url) return false;
  if (url.startsWith('javascript:') || url.startsWith('vbscript:')) return false;
  if (url.startsWith('file:')) return false;
  if (url.startsWith('data:')) {
    return allowDataImage && url.startsWith('data:image/');
  }
  return true;
}

function sanitizeAttributes(el: Element): void {
  const tag = el.tagName.toLowerCase();

  for (const attr of Array.from(el.attributes)) {
    const name = attr.name.toLowerCase();

    if (name.startsWith('on') || !ALLOWED_ATTRS.has(name)) {
      el.removeAttribute(attr.name);
      continue;
    }

    if (name === 'href' || name === 'src') {
      if (!isSafeUrl(attr.value, tag === 'img' && name === 'src')) {
        el.removeAttribute(attr.name);
      }
    }
  }

  if (tag === 'a' && el.hasAttribute('href')) {
    el.setAttribute('rel', 'noopener noreferrer');
    el.setAttribute('target', '_blank');
  }
}

function sanitizeChildren(parent: ParentNode): void {
  for (const child of Array.from(parent.childNodes)) {
    if (child.nodeType === Node.TEXT_NODE) {
      continue;
    }
    if (child.nodeType !== Node.ELEMENT_NODE) {
      // 注释、CDATA 等一律移除
      child.remove();
      continue;
    }

    const el = child as Element;
    const tag = el.tagName.toLowerCase();

    if (DANGEROUS_TAGS.has(tag)) {
      el.remove();
      continue;
    }

    if (!ALLOWED_TAGS.has(tag)) {
      // 未知标签：先清洗内部，再用子节点替换自身
      sanitizeChildren(el);
      el.replaceWith(...Array.from(el.childNodes));
      continue;
    }

    sanitizeAttributes(el);
    sanitizeChildren(el);
  }
}

/**
 * 清洗一段 HTML 字符串，返回安全的 HTML
 */
export function sanitizeHtml(html: string): string {
  if (!html) return '';

  const template = document.createElement('template');
  template.innerHTML = html;
  sanitizeChildren(template.content);
  return template.innerHTML;
}
