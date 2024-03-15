import fs from "fs-extra";
import { IaccountInfo, saveAccountInfo } from "./types/ITypes";
import Config from "../../utils/config";

export class SfacgAccountManager {
  private account: saveAccountInfo
  constructor() {
    const saveAccountPath = `${__dirname}/output/Accounts/${Config.sfacg.AppName}.json`;
    const saveAccountInfo: saveAccountInfo = fs.readJSONSync(saveAccountPath);
    this.account = new Proxy(saveAccountInfo,
      {
        set(target: saveAccountInfo, p: keyof saveAccountInfo, newValue: any, receiver) {
          target[p] = newValue;
          fs.writeJSON(saveAccountPath, target, { spaces: 2 })
            .then(() => console.log('账号已自动保存。'))
            .catch(err => console.error(`错误写入账号文件: ${err}`));
          return true
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
