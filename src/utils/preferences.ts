/**
 * 本地偏好的读写。
 *
 * 两类失败都要吞掉，且理由不同：
 * - **读**：`localStorage` 在隐私模式、被策略禁用时读取即抛异常；JSON 也可能
 *   因为历史版本或被外部改写而无法解析。两种情况都退回内置默认值，
 *   让用户以默认设置进入页面，而不是看到启动错误。
 * - **写**：磁盘配额超限时 `setItem` 抛异常。偏好没存上是可接受的降级，
 *   不影响当前会话的阅读，因此不向上报告。
 *
 * 校验规则不在这里：调用方通过 `parse` 决定「存下来的字段是否仍然合法」，
 * 因为这个判断依赖各自的选项集合（字号范围、倍速白名单等）。
 */

/**
 * 读取并校验一条 JSON 偏好。
 *
 * # 参数
 * - `key`：`localStorage` 键名。
 * - `parse`：把解析结果收敛成合法值；抛异常视为无效。
 * - `fallback`：键不存在、读取失败或校验失败时返回的默认值。
 *
 * # 返回
 * 合法的偏好对象，始终是新实例。
 */
export function readPreference<T>(
  key: string,
  parse: (raw: unknown) => T,
  fallback: () => T,
): T {
  if (typeof window === "undefined") return fallback();

  let stored: string | null;
  try {
    stored = window.localStorage.getItem(key);
  } catch {
    return fallback();
  }
  if (!stored) return fallback();

  try {
    return parse(JSON.parse(stored) as unknown);
  } catch {
    return fallback();
  }
}

/**
 * 写入一条 JSON 偏好。失败静默：受限上下文下偏好丢失不影响当前会话。
 */
export function writePreference(key: string, value: unknown): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // 配额超限或被策略禁用，保留当前会话内的设置即可。
  }
}

/**
 * 从任意 JSON 值里读一个字段，形态不符时返回 `undefined`。
 *
 * 用于把 `JSON.parse` 的结果从 `unknown` 收窄成对象，避免调用方
 * 到处写 `as Partial<T>` 断言。
 */
export function asRecord(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null
    ? (value as Record<string, unknown>)
    : {};
}
