import { createApp } from "./server/app";
import { config } from "./server/config";
import { loadRequestPolicy } from "./server/services/requestPolicy";

void loadRequestPolicy().then(() => createApp().listen(config.port, config.host, () =>
  console.log(`Novel Flow API: http://${config.host}:${config.port}`),
));
