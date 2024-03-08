import CONFIG from "../config";
import path from "path";
import fs from "fs";
interface acconutInfo{
    userName: string,
    passWord: string,
    nickName: string,
    updateAt: string
}

export default class SFTOOL {
  private accountFile?: any;

  constructor(APP_NAME: any) {
    this.accountFile = path.join(CONFIG.ACCOUNT_SAVE, `${APP_NAME}.json`);
  }

  addAccount(acconutInfo: acconutInfo) {
    let oldAccountList = this.getAccountList();
      oldAccountList.unshift(acconutInfo);
      
  }
  removeAccount(userName: string) {}

  queryAccount(account: string) {}

  private getAccountList() {
    return fs.existsSync(this.accountFile)
      ? JSON.parse(fs.readFileSync(this.accountFile, { encoding: "utf-8" }))
          .data
      : [];
  }
}
