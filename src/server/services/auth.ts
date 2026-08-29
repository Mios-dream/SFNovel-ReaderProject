import axios from "axios";
import type { Request, Response } from "express";
import fse from "fs-extra";
import path from "node:path";
import { spawn } from "node:child_process";
import WebSocket from "ws";
import { config } from "../config";
import type { AuthSession } from "../types";

type DevToolsTab = { type: string; url: string; webSocketDebuggerUrl?: string };
type DevToolsCookie = { name: string; value: string; domain: string };
type DevToolsVersion = { webSocketDebuggerUrl?: string };
type ControlledBrowserSession = { cookie: string; loginCompleted: boolean };

let browserLaunchPendingUntil = 0;

/**
 * 从受控浏览器的会话 Cookie 中恢复当前请求的 SF 登录身份。
 * @param req Express 请求对象。
 * @returns 已解析的 SF 会话；Cookie 缺失或格式无效时返回 undefined。
 */
export function getAuthSession(req: Request): AuthSession | undefined {
  const { cookieName } = config.auth;
  const token = req.headers.cookie?.split(/;\s*/)
    .find((item) => item.startsWith(`${cookieName}=`))
    ?.slice(cookieName.length + 1);
  if (!token) return undefined;
  try {
    const parsed = JSON.parse(Buffer.from(token, "base64url").toString("utf8")) as Partial<AuthSession>;
    return typeof parsed.cookie === "string" && typeof parsed.userName === "string"
      ? { cookie: parsed.cookie, userName: parsed.userName }
      : undefined;
  } catch {
    return undefined;
  }
}

/**
 * 将登录会话写入 HttpOnly Cookie。
 * @param res Express 响应对象。
 * @param session 待保存的 SF 会话信息。
 * @returns 无返回值；Cookie 会附加到响应头。
 */
export function saveAuthSession(res: Response, session: AuthSession) {
  res.cookie(config.auth.cookieName, Buffer.from(JSON.stringify(session)).toString("base64url"), {
    httpOnly: true, sameSite: "strict", maxAge: config.auth.cookieMaxAge, path: "/",
  });
}

/**
 * 清除当前客户端的登录会话 Cookie。
 * @param res Express 响应对象。
 * @returns 无返回值。
 */
export function clearAuthSession(res: Response) {
  res.clearCookie(config.auth.cookieName, { httpOnly: true, sameSite: "strict", path: "/" });
}

/**
 * 查找本机安装的 Edge 或 Chrome 可执行文件。
 * @returns 浏览器可执行文件路径，未找到时返回 undefined。
 */
function findBrowserExecutable() {
  const programFiles = process.env.ProgramFiles || "C:\\Program Files";
  const programFilesX86 = process.env["ProgramFiles(x86)"] || "C:\\Program Files (x86)";
  const candidates = [
    path.join(programFiles, "Microsoft", "Edge", "Application", "msedge.exe"),
    path.join(programFilesX86, "Microsoft", "Edge", "Application", "msedge.exe"),
    path.join(programFiles, "Google", "Chrome", "Application", "chrome.exe"),
    path.join(programFilesX86, "Google", "Chrome", "Application", "chrome.exe"),
  ];
  return candidates.find((candidate) => fse.pathExistsSync(candidate));
}

/**
 * 判断浏览器页面是否已完成官方 SF 登录跳转。
 * @param url 浏览器标签页当前 URL。
 * @returns 已完成登录时返回 true。
 */
function hasCompletedOfficialLogin(url: string) {
  try {
    const pageUrl = new URL(url);
    const host = pageUrl.hostname.toLowerCase();
    if (!host.endsWith(".sfacg.com") && host !== "sfacg.com") return false;
    return host !== "passport.sfacg.com" || (pageUrl.pathname.toLowerCase().endsWith("/message.aspx") && pageUrl.searchParams.get("msg") === "LoginSuccessToHome");
  } catch { return false; }
}

/**
 * 通过 DevTools Protocol 读取受控浏览器中的 SF Cookie。
 * @returns 已提取的浏览器会话，无法读取时返回 undefined。
 */
async function getControlledBrowserSession(): Promise<ControlledBrowserSession | undefined> {
  // 通过 Chrome DevTools Protocol 读取官方登录窗口 Cookie，避免自行模拟登录流程。
  try {
    const { data } = await axios.get<DevToolsTab[]>(`http://127.0.0.1:${config.auth.browserDebugPort}/json/list`, { timeout: 700 });
    const sfPages = data.filter((tab) => tab.type === "page" && tab.url.includes("sfacg.com") && tab.webSocketDebuggerUrl);
    const page = sfPages.find((tab) => hasCompletedOfficialLogin(tab.url)) || sfPages[0];
    if (!page?.webSocketDebuggerUrl) return undefined;
    const cookies = await new Promise<DevToolsCookie[]>((resolve, reject) => {
      const socket = new WebSocket(page.webSocketDebuggerUrl!);
      const timeout = setTimeout(() => { socket.terminate(); reject(new Error("读取官方登录会话超时")); }, 2500);
      socket.once("open", () => socket.send(JSON.stringify({ id: 1, method: "Network.getAllCookies" })));
      socket.on("message", (raw) => {
        try {
          const message = JSON.parse(raw.toString()) as { id?: number; result?: { cookies?: DevToolsCookie[] } };
          if (message.id === 1) { clearTimeout(timeout); socket.close(); resolve(message.result?.cookies || []); }
        } catch (error) { clearTimeout(timeout); socket.terminate(); reject(error); }
      });
      socket.once("error", (error) => { clearTimeout(timeout); reject(error); });
    });
    const sfCookies = cookies.filter((cookie) =>
      (cookie.domain === "sfacg.com" || cookie.domain === ".sfacg.com" || cookie.domain === "api.sfacg.com") &&
      (cookie.name === ".SFCommunity" || cookie.name.startsWith("session_") || cookie.name.startsWith(".SF")),
    );
    if (!sfCookies.some((cookie) => cookie.name === ".SFCommunity")) return undefined;
    const cookie = [...new Map(sfCookies.sort((left, right) => Number(right.domain === "api.sfacg.com") - Number(left.domain === "api.sfacg.com")).map((item) => [item.name, item.value])).entries()]
      .map(([name, value]) => `${name}=${value}`).join("; ");
    return { cookie, loginCompleted: hasCompletedOfficialLogin(page.url) };
  } catch { return undefined; }
}

/**
 * 启动带远程调试端口的官方登录浏览器窗口。
 * @returns 浏览器进程启动完成后的 Promise。
 * @throws 未找到支持的浏览器时抛出错误。
 */
export async function startBrowserLogin() {
  if (await isControlledBrowserRunning()) return;
  const executable = findBrowserExecutable();
  if (!executable) throw new Error("未找到 Microsoft Edge 或 Google Chrome");
  await fse.ensureDir(config.auth.browserProfileDir);
  const browser = spawn(executable, [
    `--remote-debugging-port=${config.auth.browserDebugPort}`, "--remote-debugging-address=127.0.0.1",
    "--remote-allow-origins=*", `--user-data-dir=${config.auth.browserProfileDir}`,
    "--no-first-run", "--no-default-browser-check", "--new-window", config.auth.browserLoginUrl,
  ], { detached: true, stdio: "ignore", windowsHide: false });
  browser.unref();
  browserLaunchPendingUntil = Date.now() + 12_000;
}

/**
 * 检查登录是否完成，完成后保存会话并关闭受控浏览器。
 * @returns 已完成的浏览器会话；登录尚未完成时返回 undefined。
 */
export async function completeBrowserLogin(): Promise<ControlledBrowserSession | undefined> {
  const session = await getControlledBrowserSession();
  if (!session?.loginCompleted) return undefined;
  browserLaunchPendingUntil = 0;
  await closeControlledBrowser();
  return session;
}

/**
 * 判断登录浏览器是否正在运行或刚刚启动。
 * @returns 仍在等待用户登录时返回 true。
 */
export async function isBrowserLoginWaiting() {
  return (await isControlledBrowserRunning()) || Date.now() < browserLaunchPendingUntil;
}

/**
 * 查询受控浏览器的 DevTools 调试端口是否可用。
 * @returns 浏览器可连接时返回 true。
 */
async function isControlledBrowserRunning() {
  try { await axios.get(`http://127.0.0.1:${config.auth.browserDebugPort}/json/version`, { timeout: 500 }); return true; }
  catch { return false; }
}

/**
 * 通过 DevTools Protocol 关闭受控浏览器。
 * @returns 浏览器关闭完成后的 Promise。
 */
async function closeControlledBrowser() {
  try {
    const { data } = await axios.get<DevToolsVersion>(`http://127.0.0.1:${config.auth.browserDebugPort}/json/version`, { timeout: 700 });
    if (!data.webSocketDebuggerUrl) return;
    await new Promise<void>((resolve) => {
      const socket = new WebSocket(data.webSocketDebuggerUrl!);
      const timeout = setTimeout(() => { socket.terminate(); resolve(); }, 1500);
      socket.once("open", () => socket.send(JSON.stringify({ id: 1, method: "Browser.close" })));
      socket.once("message", () => { clearTimeout(timeout); socket.close(); resolve(); });
      socket.once("error", () => { clearTimeout(timeout); resolve(); });
    });
  } catch { /* browser was already closed */ }
}
