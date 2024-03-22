
import readlineSync from 'readline-sync';
import { IsearchInfos } from "./types/ITypes";
import { SfacgAccountManager } from "./account";
import { SfacgClient, SfacgRegister } from "./client";
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

    async Once() {
        const userName = readlineSync.question('输入账号：');
        const passWord = readlineSync.question('输入密码：', {
            hideEchoBack: true
        });
        this.client.login(userName, passWord)
        const a = readlineSync.keyIn("[1]直接搜书\n[2]书架选书", { limit: '12' })
        switch (a) {
            case "1":
                const bookname = readlineSync.question("请输入书名：")
                const books = await this.client.searchInfos(bookname)
                const id = books ? this.selectBookFromList(books) : books
                id && this.client.volumeInfos(id)
                
                break
            case "2":


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

    async downLoadBook(bookId: number) {
        const volunms = await this.client.volumeInfos(bookId)

    }

    selectBookFromList(books: IsearchInfos[]): number {
        books.forEach((book, index) => {
            console.log(`[${index + 1}] ${book.novelName} - ${book.authorId}`);
        });
        const bookIndex = readlineSync.question("请输入书序号选择：", {
            limit: new RegExp(`^[1-${books.length}]$`) // 限制用户只能输入在书列表范围内的数字
        });
        return books[parseInt(bookIndex) - 1].novelId;
    }
}