import express from "express";
import { config } from "./config";
import { authRouter } from "./routes/auth";
import { catalogRouter } from "./routes/catalog";
import { jobsRouter } from "./routes/jobs";
import { libraryRouter } from "./routes/library";

export function createApp() {
  const app = express();
  app.use(express.json());
  app.use("/library", express.static(config.libraryDir));
  app.use("/api", catalogRouter);
  app.use("/api/auth", authRouter);
  app.use("/api", jobsRouter);
  app.use("/api", libraryRouter);
  app.get("/api/health", (_req, res) => res.json({ ok: true }));
  app.use(express.static(config.webDir));
  return app;
}
