import { Router } from "express";
import { refreshDownloadSlots } from "../services/jobs";
import { getRequestPolicy, updateRequestPolicy } from "../services/requestPolicy";

export const settingsRouter = Router();

settingsRouter.get("/request-policy", (_req, res) => res.json(getRequestPolicy()));

settingsRouter.put("/request-policy", async (req, res) => {
  try {
    const policy = await updateRequestPolicy(req.body || {});
    refreshDownloadSlots();
    res.json(policy);
  } catch (error) {
    res.status(400).json({ message: error instanceof Error ? error.message : "请求策略无效" });
  }
});
