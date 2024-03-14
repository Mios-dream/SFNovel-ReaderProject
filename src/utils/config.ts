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
  sfacg: SfacgOption;
}
export interface Base {
  proxy?: string;
  orderedChaps?: boolean;
}

// 配置文件路径
const CONFIG_PATH = path.join(__dirname, "../config.json");

export class ProxyHandler<T extends Record<string, any>> {
  private configPath: string;
  private config: T;

  constructor(configPath: string) {
    this.configPath = configPath;
    this.config = this.loadConfig();
  }

  // 加载配置文件
  private loadConfig(): T {
    try {
      return fs.readJSONSync(this.configPath);
    } catch (error) {
      throw new Error(`无法加载配置文件: ${error}`);
    }
  }

  createProxy(): T {
    return new Proxy<T>(this.config, {
      set: (target: T, property: string | symbol, value: any): boolean => {
        if (typeof property === 'string') {
          const updatedTarget = { ...target, [property]: value };
          // 异步写入配置到文件
          fs.writeJson(this.configPath, updatedTarget, { spaces: 2 })
            .then(() => console.log('配置已自动保存。'))
            .catch(err => console.error(`错误写入配置文件: ${err}`));
          this.config = updatedTarget;
        }
        return true;
      },
    });
  }
}

const configHandler = new ProxyHandler(CONFIG_PATH);
const Config = configHandler.createProxy();

export default Config;
