import fs from "fs-extra";
import path from "path";
import { SfacgOption } from "../client/Sfacg/types/ITypes";


export interface Register {
  userName: string;
  passWord: string;
  token: string
}
export interface AppConfig {
  version: string;
  Register: Register;
  Sfacg: SfacgOption;
}
export interface Base {
  proxy?: string;
  orderedChaps?: boolean;
}

// 配置文件路径
const CONFIG_PATH = path.resolve(__dirname, "../config.json");

// Still in optimization, waitting to add the support for AcoountManager && smsService 
function loadConfig() {
  try {
    return fs.readJSONSync(CONFIG_PATH);
  } catch (error) {
    throw new Error(`无法加载配置文件: ${error}`);
  }
};

const Config: AppConfig = new Proxy(loadConfig(), {
  set: (target: AppConfig, property: keyof AppConfig, value: any): boolean => {
    if (typeof property === 'string') {
      target[property] = value
      // 异步写入配置到文件
      fs.writeJson(CONFIG_PATH, target, { spaces: 2 })
        .then(() => console.log('配置已自动保存。'))
        .catch(err => console.error(`错误写入配置文件: ${err}`));
    }
    return true;
  }
});

export default Config;
