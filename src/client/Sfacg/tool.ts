import CONFIG from "../config";
import path from "path";
import fs from "fs";


export default class SfacgTool {
  private accountFile?: any;

  constructor(APP_NAME: any) {
    this.accountFile = path.join(CONFIG.ACCOUNT_SAVE, `${APP_NAME}.json`);
  }

  addAccount(acconutInfo: any) {
    let oldAccountList = this.getAccountList();
      oldAccountList.unshift(acconutInfo);
      
  }
  removeAccount(userName: string) {
    
  }

  queryAccount(account: string) {}

  private getAccountList() {
    return fs.existsSync(this.accountFile)
      ? JSON.parse(fs.readFileSync(this.accountFile, { encoding: "utf-8" }))
          .data
      : [];
  }
}
