import { SfacgOption } from "../client/Sfacg/types/ITypes";
import config from "../config.json";

export interface Register {
  userName: string;
  passWord: string;
}

export interface AppConfig {
  version: string;
  Register: Register;
  sfacg: SfacgOption;
}

export interface Base {
  proxy?: string;
  orderedChaps?: boolean;
  novelOutput?: string;
}

const Config: AppConfig = {
  version: config.version || "1.0.0",
  Register: config.Register,
  sfacg: {
    ...config.Base,
    ...config.Sfacg,
  },
};
export default Config;
