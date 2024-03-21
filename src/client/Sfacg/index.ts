import { initial, times } from "lodash";
import readlineSync from 'readline-sync';
import { SfacgAccountManager, SfacgClient, SfacgRegister } from "./client";
// const baseinfo = await this.userInfo()
// const money = await this.userMoney()
// const cookie: string = this.cookieJar.getCookieStringSync(SfacgHttp.HOST)
// const acconutInfo = {
//   userName: userName,
//   passWord: passWord,
//   cookie: cookie,
//   ...baseinfo,
//   ...money,
// };


export class Sfacg {

    client: SfacgClient
    accountManager: SfacgAccountManager
    register: SfacgRegister

    constructor() {
        this.client = new SfacgClient()
        this.accountManager = new SfacgAccountManager()
        this.register = new SfacgRegister()
    }

    init() {
        const answer = readlineSync.keyIn('[1]帮人提书\n[2]每日奖励\n[3]账号管理\n[4]多账号提书\n[5]注册机启动：\n选择一个选项:', {
            limit: '12345'
        })
        switch (answer) {
            case "1":
                this.Once()
                break
            case "2":
                this.Bonus()
                break
            case "3":
                this.Account()
                break
            case "4":
                this.Multi()
                break
            case "5":
                this.Regist()
                break
        }
    }

    Once() {
        const userName = readlineSync.question('输入账号：');
        const passWord = readlineSync.question('输入密码：', {
            hideEchoBack: true
        });
        this.client.login(userName, passWord)
        const a = readlineSync.keyIn("[1]直接搜书\n[2]书架选书")
        switch (a) {
            case "1":
                this.client.searchInfos("1")
        }

    }

    Regist() {

    }

    Bonus() {

    }

    Multi() {

    }

    Account() {

    }
}