import path from "path";
import fs from "fs";
import Config from "./config";
import { IaccountInfo } from "../client/Sfacg/types/ITypes";

export class SfacgTool {
  saveAccountInfo: any;
  constructor() {
    
  }
  addAccount(acconutInfo: IaccountInfo) {}
  removeAccount(userName: string) {}
  queryAccount(account: string) {}
  getAccountList() {}
}
