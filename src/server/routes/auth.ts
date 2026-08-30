import { Router } from "express";
import {
  clearAuthSession,
  completeBrowserLogin,
  getAuthSession,
  isBrowserLoginWaiting,
  saveAuthSession,
  startBrowserLogin,
  loginWithPassword,
} from "../services/auth";
import { SfacgApiClient } from "../infrastructure/sfacg/client";
import { config } from "../config";
import { cached, sessionKey } from "../services/cache";

export const authRouter = Router();

/** 默认的账号密码登录入口；密码仅在本次请求内存中使用。 */
authRouter.post("/login", async (req, res) => {
  const username = typeof req.body?.username === "string" ? req.body.username.trim() : "";
  const password = typeof req.body?.password === "string" ? req.body.password : "";
  if (!username || !password)
    return res.status(400).json({ message: "请输入 SF 账号和密码" });
  try {
    const session = await loginWithPassword(username, password);
    saveAuthSession(res, session);
    res.json({ authenticated: true, userName: session.userName });
  } catch (error) {
    res.status(401).json({ message: error instanceof Error ? error.message : "SF 账号密码登录失败" });
  }
});

/**
 * 返回当前请求关联的登录状态。
 * @param req 包含会话 Cookie 的 Express 请求。
 * @param res 返回登录状态的 Express 响应。
 * @returns 无返回值。
 */
authRouter.get("/status", (req, res) => {
  const session = getAuthSession(req);
  res.json({ authenticated: Boolean(session), userName: session?.userName });
});

/**
 * 返回当前登录账号的昵称、头像和账户余额。
 * @param req 包含会话 Cookie 的 Express 请求。
 * @param res 返回用户资料或读取失败信息的 Express 响应。
 * @returns 用户资料读取完成后的 Promise。
 */
authRouter.get("/profile", async (req, res) => {
  const session = getAuthSession(req);
  if (!session)
    return res.status(401).json({ message: "请先登录 SF 账号" });
  try {
    // 账户资料依赖登录会话：按 Cookie 隔离缓存，且同一时刻只允许一次上游加载。
    const profile = await cached(
      sessionKey(session.cookie, "profile"),
      config.cache.profileTtl,
      async () => {
        const client = new SfacgApiClient();
        client.setCookie(session.cookie);
        client.setNonce(session.nonce);
        const result = await client.userProfile();
        if (!result)
          throw new Error("无法读取账户资料，SF 登录会话可能已失效");
        return result;
      },
    );
    saveAuthSession(res, { ...session, userName: profile.nickName });
    res.json(profile);
  } catch (error) {
    res.status(502).json({
      message: error instanceof Error ? error.message : "读取账户资料失败",
    });
  }
});

/**
 * 启动官方登录浏览器窗口。
 * @param _req 未使用的 Express 请求。
 * @param res 返回等待状态或启动错误的 Express 响应。
 * @returns 启动处理完成后的 Promise。
 */
authRouter.post("/browser-login", async (_req, res) => {
  try {
    await startBrowserLogin();
    res.status(202).json({ status: "waiting" });
  } catch (error) {
    res.status(500).json({
      message: error instanceof Error ? error.message : "无法启动登录浏览器",
    });
  }
});

/**
 * 轮询官方浏览器登录结果，并在成功后写入会话 Cookie。
 * @param _req 未使用的 Express 请求。
 * @param res 返回登录状态的 Express 响应。
 * @returns 登录检查完成后的 Promise。
 */
authRouter.get("/browser-login/status", async (_req, res) => {
  const session = await completeBrowserLogin();
  if (!session)
    return res.json({
      authenticated: false,
      waiting: await isBrowserLoginWaiting(),
    });
  saveAuthSession(res, { cookie: session.cookie, userName: "已登录 SF 账号" });
  res.json({ authenticated: true, userName: "已登录 SF 账号" });
});

/**
 * 清除当前客户端的登录会话。
 * @param _req 未使用的 Express 请求。
 * @param res 写入清除 Cookie 响应的 Express 响应。
 * @returns 无返回值。
 */
authRouter.post("/logout", (_req, res) => {
  clearAuthSession(res);
  res.status(204).end();
});
