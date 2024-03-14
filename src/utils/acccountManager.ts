import fs from "fs";
import { IaccountInfo } from "../client/Sfacg/types/ITypes";
import Config from "./config";

export class SfacgAccountManager {
  saveAccountInfo: any;
  saveAccountPath: any;
  constructor() {
    const _path = process.cwd()
    this.saveAccountPath = `${_path}/Accounts/${Config.sfacg.}`;
    this.saveAccountInfo = fs.readFileSync();
  }
  addAccount(acconutInfo: IaccountInfo) {}
  removeAccount(userName: string) {}
  queryAccount(account: string) {}
  getAccountList() {}
}
