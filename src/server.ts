import { createApp } from "./server/app";
import { config } from "./server/config";

createApp().listen(config.port, config.host, () =>
  console.log(`Novel Flow API: http://${config.host}:${config.port}`),
);
