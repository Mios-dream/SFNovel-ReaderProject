import { Router } from "express";
import { clearAuthSession, completeBrowserLogin, getAuthSession, isBrowserLoginWaiting, saveAuthSession, startBrowserLogin } from "../services/auth";

export const authRouter = Router();

authRouter.get("/status", (req, res) => {
  const session = getAuthSession(req);
  res.json({ authenticated: Boolean(session), userName: session?.userName });
});

authRouter.post("/browser-login", async (_req, res) => {
  try { await startBrowserLogin(); res.status(202).json({ status: "waiting" }); }
  catch (error) { res.status(500).json({ message: error instanceof Error ? error.message : "无法启动登录浏览器" }); }
});

authRouter.get("/browser-login/status", async (_req, res) => {
  const session = await completeBrowserLogin();
  if (!session) return res.json({ authenticated: false, waiting: await isBrowserLoginWaiting() });
  saveAuthSession(res, { cookie: session.cookie, userName: "已登录 SF 账号" });
  res.json({ authenticated: true, userName: "已登录 SF 账号" });
});

authRouter.post("/logout", (_req, res) => { clearAuthSession(res); res.status(204).end(); });
