
import fs from "fs-extra"
import { IaccountInfo } from "./types/ITypes";
import path from "path"
import { saveAccountInfo } from "../../utils/types/types";

export class SfacgAccountManager {
    saveAccountPath: string
    account: saveAccountInfo
    constructor() {
        // 待改成数据库
        this.saveAccountPath = path.resolve(__dirname, `../output/Accounts/SfacgAccount.json`)
        if (!fs.pathExistsSync(this.saveAccountPath)) {
            fs.outputJSONSync(this.saveAccountPath, { data: [] })
        }
        const saveAccountInfo: any = fs.readJSONSync(this.saveAccountPath);
        this.account = saveAccountInfo
    }
    
    cookieGet(userName: string) {
        const index = this.findAccountIndex(userName)
        if (index !== -1) {
            return this.account.data[index].cookie
        }
    }

    cookieSave(userName: string, newCookie: string) {
        const index = this.findAccountIndex(userName)
        if (index !== -1) {
            this.account.data[index].cookie = newCookie
            fs.writeJSONSync(this.saveAccountPath, this.account)
        }
    }

    addCheckInfo(userName: string) {
        const index = this.findAccountIndex(userName)
        if (index !== -1) {
            this.account.data[index].lastCheckIn = new Date()
            fs.writeJSONSync(this.saveAccountPath, this.account)
        }
    }

    addAccount(acconutInfo: IaccountInfo) {
        this.account.data.push(acconutInfo)
        fs.writeJSONSync(this.saveAccountPath, this.account)
    }

    removeAccount(userName: string) {
        const index = this.findAccountIndex(userName)
        if (index !== -1) {
            this.account.data.splice(index, 1);
        }
        fs.writeJSONSync(this.saveAccountPath, this.account)
    }

    getAccountList() {
        return this.account.data.map(({ userName, fireMoneyRemain = 0, couponsRemain = 0 }: any, index: number) =>
            `${index + 1}.${userName} 余额： ${fireMoneyRemain + couponsRemain}`
        ).join('\n');
    }


    findAccountIndex(userName: string) {
        const index = this.account.data.findIndex((account: { userName: string; }) => {
            account.userName == userName
        })
        return index
    }

}