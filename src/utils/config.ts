import fs from "fs";
import path from "path";
import { SfacgOption } from "../client/Sfacg/types/ITypes";

// 假设这些接口已经在其他地方定义
export interface Register {
  userName: string;
  passWord: string;
  token: string
}

export interface AppConfig {
  version: string;
  Register: Register;
  sfacg: SfacgOption;
}
export interface Base {
  proxy?: string;
  orderedChaps?: boolean;
}

// 配置文件路径
const CONFIG_PATH = path.join(__dirname, "../config.json");

// 加载配置文件
function loadConfig(): AppConfig {
  try {
    const configText = fs.readFileSync(CONFIG_PATH, { encoding: "utf-8" });
    return JSON.parse(configText);
  } catch (error) {
    throw new Error(`无法加载配置文件: ${error}`);
  }
}

// 创建一个代理来自动保存更改到配置文件
const Config: AppConfig = new Proxy(loadConfig(), {
  set(target: AppConfig, property: keyof AppConfig, value: any): boolean {
    target[property] = value;
    // 异步写入配置到文件
    fs.writeFile(CONFIG_PATH, JSON.stringify(target, null, 2), (err) => {
      if (err) {
        console.error(`错误写入配置文件: ${err}`);
        return false;
      }
    });
    return true;
  },
});

export default Config;
