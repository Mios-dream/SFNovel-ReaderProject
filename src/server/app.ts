import express from "express";
import { config } from "./config";
import { authRouter } from "./routes/auth";
import { catalogRouter } from "./routes/catalog";
import { jobsRouter } from "./routes/jobs";
import { libraryRouter } from "./routes/library";
import { settingsRouter } from "./routes/settings";

/**
 * 创建并组装 API、静态资源和业务路由。
 * @returns 配置完成但尚未监听端口的 Express 应用实例。
 */
export function createApp() {
  const app = express();
  app.use(express.json());
  // 书库以静态资源形式暴露，前端可直接打开已生成的 Markdown/音频文件。
  app.use("/library", express.static(config.libraryDir));
  app.use("/api", catalogRouter);
  app.use("/api/auth", authRouter);
  app.use("/api", jobsRouter);
  app.use("/api", libraryRouter);
  app.use("/api/settings", settingsRouter);
  app.get("/api/health", (_req, res) => res.json({ ok: true }));
  app.use(express.static(config.webDir));
  return app;
}
