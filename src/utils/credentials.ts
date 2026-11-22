import type { CredentialKind, LoginCredentials } from "../types";

const APP_SESSION = /\bsession_APP\b/;
const WEB_SESSION = /\bsession_PC\b/;

/**
 * 把凭据序列化成一段 JSON。
 *
 * 只带 Cookie 与设备编号，不带类型：接收方看 Cookie 里有没有 `session_APP` 或
 * `session_PC` 就知道该写回哪一侧，省得两个地方各存一份类型、还可能对不上。
 */
export function formatLoginCredentials(credentials: LoginCredentials): string {
  return JSON.stringify(
    {
      cookie: credentials.cookie,
      ...(credentials.deviceId ? { deviceId: credentials.deviceId } : {}),
    },
    null,
    2,
  );
}

/**
 * 解析本应用导出的凭据 JSON。
 */
export function parseLoginCredentials(text: string): LoginCredentials {
  let payload: unknown;
  try {
    payload = JSON.parse(text);
  } catch {
    return { cookie: "", deviceId: "" };
  }
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
    return { cookie: "", deviceId: "" };
  }
  const record = payload as Record<string, unknown>;
  const cookie = typeof record.cookie === "string" ? record.cookie.trim() : "";
  const deviceId =
    typeof record.deviceId === "string" ? record.deviceId.trim() : "";
  return { cookie, deviceId };
}

/**
 * 按 Cookie 里的会话键判断凭据类型。
 *
 * 与后端判定顺序一致：先认 `session_APP`。App 导出的 Cookie 是完整的，可能同时
 * 带 `session_PC`，这时它就是 App 会话。
 *
 * @returns 两个会话键都没有时返回 `undefined`，交给后端报「无法判断登录类型」。
 */
export function credentialKind(cookie: string): CredentialKind | undefined {
  if (APP_SESSION.test(cookie)) return "app";
  if (WEB_SESSION.test(cookie)) return "web";
  return undefined;
}
