import fs from "fs";
import { IaccountInfo, saveAccountInfo } from "./types/ITypes";
import Config from "../../utils/config";

export class SfacgAccountManager {
  private account: saveAccountInfo
  constructor() {
    const saveAccountPath = `${__dirname}/Accounts/${Config.sfacg.AppName}.json`;
    const saveAccountInfo: saveAccountInfo = JSON.parse(fs.readFileSync(saveAccountPath, "utf-8"));
    this.account = new Proxy(saveAccountInfo,
      {
        set(target: saveAccountInfo, p: keyof saveAccountInfo, newValue: any, receiver) {
          target[p] = newValue;
          fs.writeFile(saveAccountPath, JSON.stringify(target, null, 2), (err) => {
            if (err) {
              console.error(`错误写入账号: ${err}`);
              return false;
            }
          });
          return true;
        },
      })
  }

  addAccount(acconutInfo: IaccountInfo) {
    this.account.data.push(acconutInfo)
  }

  removeAccount(userName: string) {
    this.account.data
  }
  queryAccount(account: string) { }
  getAccountList() { }
}
