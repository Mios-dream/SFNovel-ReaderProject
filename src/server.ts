import { createApp } from "./server/app";
import { config } from "./server/config";
import { loadRequestPolicy } from "./server/services/requestPolicy";

// 请求策略加载完成后再启动 HTTP 服务，避免首批下载任务使用默认限流参数。
void loadRequestPolicy().then(() =>
  createApp().listen(config.port, config.host, () =>
    console.log(`Novel Flow API: http://${config.host}:${config.port}`),
  ),
);
