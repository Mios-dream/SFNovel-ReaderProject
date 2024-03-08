import dotenv from "dotenv";

dotenv.config();

interface ENV {
  VERSION: string;
  PROXY: URL | string;
  ACCOUNT_SAVE: string;
  NOVEL_OUTPUT: string;
  SFACG_APPNAME: string;
  CIWEIMAO_APPNAME: string;
}

const CONFIG: ENV = {
  VERSION: process.env.VERSION || "1.0.0",
  PROXY: process.env.PROXY || "",
  ACCOUNT_SAVE: process.env.ACCOUNT_SAVE || "Account",
  NOVEL_OUTPUT: process.env.NOVEL_OUTPUT || "Novels",
  SFACG_APPNAME: process.env.SFACG_APPNAME || "Sfacg",
  CIWEIMAO_APPNAME: process.env.SFACG_APPNAME || "Ciweimao",
};

export default CONFIG;
