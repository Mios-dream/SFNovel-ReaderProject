/**
 * 文本折行工具。
 *
 * 使用 Canvas 的 `measureText` 做纯计算，不接触真实 DOM 布局，
 * 因此不会触发强制回流。算法采用逐 token 累积的贪心策略：
 * - 中文/标点：任意字符间可断行；
 * - 拉丁字母与数字：保持单词完整，超长单词再按字符硬拆；
 * - 空格、全角空格归入前一个 token，使其可以悬在行尾；
 * - 简化版禁则处理：避免收尾标点出现在行首、开引号出现在行尾。
 */

/** 可视为空白的字符（含全角空格）。 */
const SPACE_PATTERN = /[ \t\u3000]/;

/** 拉丁单词字符：字母与数字。 */
const WORD_PATTERN = /[A-Za-z0-9]/;

/** 不应出现在行首的收尾类标点。 */
const NO_LINE_START = new Set(
  Array.from(
    "!%),.:;?]}\u3001\u3002\uff0c\uff0e\uff1a\uff1b\uff1f\uff01\uff09\u300d\u300f\u3011\u2019\u201d\uff5d\u3015\u203a",
  ),
);

/** 不应出现在行尾的开引类标点。 */
const NO_LINE_END = new Set(
  Array.from("([{\u3008\u300a\u300c\u300e\uff08\u3010\u2018\u201c\uff3b\uff5b\u3014\u2039"),
);

let sharedContext: CanvasRenderingContext2D | undefined;

/** 获取一个可复用的 2D 上下文，专门用于文本测量。 */
export function measureContext(): CanvasRenderingContext2D {
  if (!sharedContext) {
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("无法创建用于文本测量的 Canvas 上下文");
    sharedContext = ctx;
  }
  return sharedContext;
}

/** 判断相邻两字符之间是否允许断行。 */
function canBreakBetween(previous: string, current: string): boolean {
  if (SPACE_PATTERN.test(previous) || SPACE_PATTERN.test(current)) return true;
  if (NO_LINE_START.has(current)) return false;
  if (NO_LINE_END.has(previous)) return false;
  if (WORD_PATTERN.test(previous) && WORD_PATTERN.test(current)) return false;
  return true;
}

/** 将一行文本切分为不可再分的断行 token。 */
function tokenize(text: string): string[] {
  const chars = Array.from(text);
  const tokens: string[] = [];
  let current = "";
  for (let index = 0; index < chars.length; index += 1) {
    const char = chars[index];
    if (current.length > 0) {
      const previous = chars[index - 1];
      // 空格归入前一个 token；其余情况按断行规则切分。
      const boundary = SPACE_PATTERN.test(char)
        ? false
        : SPACE_PATTERN.test(previous)
          ? true
          : canBreakBetween(previous, char);
      if (boundary) {
        tokens.push(current);
        current = "";
      }
    }
    current += char;
  }
  if (current.length > 0) tokens.push(current);
  return tokens;
}

/**
 * 将单行文本（不含换行符）按最大宽度折成多行。
 * 至少返回一个元素，空文本返回 `[""]`。
 */
export function wrapLine(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxWidth: number,
  font: string,
): string[] {
  ctx.font = font;
  const lines: string[] = [];
  let line = "";
  let lineWidth = 0;

  const flush = () => {
    lines.push(line.replace(/[ \t\u3000]+$/, ""));
    line = "";
    lineWidth = 0;
  };

  for (const token of tokenize(text)) {
    // 丢弃行首空白，避免折行后出现缩进错位。
    if (line === "" && SPACE_PATTERN.test(token)) continue;
    const tokenWidth = ctx.measureText(token).width;
    if (lineWidth + tokenWidth <= maxWidth) {
      line += token;
      lineWidth += tokenWidth;
      continue;
    }
    if (line !== "") flush();
    if (tokenWidth <= maxWidth) {
      line = token;
      lineWidth = tokenWidth;
      continue;
    }
    // 单个 token 超过整行宽度（如超长网址）：按字符硬拆。
    for (const char of Array.from(token)) {
      const charWidth = ctx.measureText(char).width;
      if (lineWidth + charWidth > maxWidth && line !== "") flush();
      line += char;
      lineWidth += charWidth;
    }
  }

  if (line !== "") flush();
  else if (lines.length === 0) lines.push("");
  return lines;
}

/**
 * 折行一段可能包含换行的文本，保留原始换行结构
 * （等价于 CSS 的 `white-space: pre-wrap`）。
 */
export function wrapParagraph(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxWidth: number,
  font: string,
): string[] {
  const result: string[] = [];
  for (const logicalLine of text.split(/\r?\n/)) {
    if (logicalLine === "") {
      result.push("");
      continue;
    }
    result.push(...wrapLine(ctx, logicalLine, maxWidth, font));
  }
  return result;
}
