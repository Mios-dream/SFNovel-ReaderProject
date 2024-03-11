import dotenv from "dotenv";

dotenv.config();

interface ENV {
  VERSION: string;
  PROXY: string;
  SAVE_ACCOUNT: boolean;
  ORDERD_CHAPS: boolean;
  EPUB_MAKE: boolean;
  ACCOUNT_SAVEFILE: string;
  NOVEL_OUTPUT: string;
  SFACG_APPNAME: string;
  CIWEIMAO_APPNAME: string;
}

const CONFIG: ENV = {
  VERSION: process.env.VERSION || "1.0.0",
  PROXY: process.env.PROXY || "",
  SAVE_ACCOUNT: Boolean(process.env.SAVE_ACCOUNT) || false,
  ORDERD_CHAPS: Boolean(process.env.ORDERD_CHAPS) || false,
  EPUB_MAKE: Boolean(process.env.EPUB_MAKE) || false,
  ACCOUNT_SAVEFILE: process.env.ACCOUNT_SAVEFILE || "Account",
  NOVEL_OUTPUT: process.env.NOVEL_OUTPUT || "Novels",
  SFACG_APPNAME: process.env.SFACG_APPNAME || "Sfacg",
  CIWEIMAO_APPNAME: process.env.SFACG_APPNAME || "Ciweimao",
};



export default CONFIG;
