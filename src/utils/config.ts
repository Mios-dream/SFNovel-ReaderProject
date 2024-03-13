import { SfacgOption } from "../client/Sfacg/types/ITypes";
import { promises as fsPromises } from "fs";

export interface AppConfig {
  version: string;
  sfacg: SfacgOption;
}

export interface Base {
  proxy?: string;
  orderedChaps?: boolean;
  accountSavefile?: string;
  novelOutput?: string;
}

export async function loadConfig(): Promise<AppConfig> {
  const _path = process.cwd();
  const jsonString = await fsPromises.readFile(`${_path}/src/config.json`, "utf-8");
  const config = JSON.parse(jsonString);
  return {
    version: config.version || "1.0.0",
    sfacg: {
      ...config.Base,
      ...config.Sfacg,
    },
  };
}
