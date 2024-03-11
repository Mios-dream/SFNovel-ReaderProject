import { SfacgOption } from "./Sfacg/types/ITypes";

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

export const Config: AppConfig = {
  version: process.env.VERSION || "1.0.0",
  sfacg: {
    proxy: process.env.PROXY || "",
    saveAccount: Boolean(process.env.SAVE_ACCOUNT) || false,
    orderedChaps: Boolean(process.env.ORDERD_CHAPS) || false,
    epubMake: Boolean(process.env.EPUB_MAKE) || false,
    accountSavefile: process.env.ACCOUNT_SAVEFILE || "Account",
    novelOutput: process.env.NOVEL_OUTPUT || "Novels",
    sfacgAppName: process.env.SFACG_APPNAME || "Sfacg",
  },
  // ciweimao: {
  //   proxy: process.env.PROXY || "",
  //   saveAccount: Boolean(process.env.SAVE_ACCOUNT) || false,
  //   orderedChaps: Boolean(process.env.ORDERD_CHAPS) || false,
  //   epubMake: Boolean(process.env.EPUB_MAKE) || false,
  //   accountSavefile: process.env.ACCOUNT_SAVEFILE || "Account",
  //   novelOutput: process.env.NOVEL_OUTPUT || "Novels",
  //   ciweimaoAppName: process.env.CIWEIMAO_APPNAME || "Ciweimao",
  // },
};
