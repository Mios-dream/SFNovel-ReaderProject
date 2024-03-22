import { IaccountInfo, IsearchInfos } from "./types/ITypes.js";
import { SfacgAccountManager } from "./account.js";
import { SfacgClient, SfacgRegister } from "./client.js";
import { SfacgHttp } from "./basehttp.js";
import inquirer from "inquirer";

// 然后，你可以像之前一样使用inquirer

export class Sfacg {
    client: SfacgClient;
    accountManager: SfacgAccountManager;
    register: SfacgRegister;

    constructor() {
        this.client = new SfacgClient();
        this.accountManager = new SfacgAccountManager();
        this.register = new SfacgRegister();
    }

    init() {
        inquirer
            .prompt([
                {
                    type: "list",
                    name: "option",
                    message: "选择一个选项:",
                    choices: [
                        { name: "帮人提书", value: 1 },
                        { name: "每日奖励", value: 2 },
                        { name: "账号管理", value: 3 },
                        { name: "多账号提书", value: 4 },
                        { name: "注册机启动！", value: 5 },
                    ],
                },
            ])
            .then((answer: any) => {
                switch (answer.option) {
                    case 1:
                        this.Once();
                        break;
                    case 2:
                        this.Bonus();
                        break;
                    case 3:
                        this.Account();
                        break;
                    case 4:
                        this.Multi();
                        break;
                    case 5:
                        this.Regist();
                        break;
                }
            });
    }

    async Once() {
        const userNameAnswer = await inquirer.prompt({
            type: "input",
            name: "userName",
            message: "输入账号：",
        });
        const passWordAnswer = await inquirer.prompt({
            type: "password",
            name: "passWord",
            message: "输入密码：",
            mask: "*",
        });
        this.client.login(userNameAnswer.userName, passWordAnswer.passWord);

        const aAnswer = await inquirer.prompt({
            type: "list",
            name: "option",
            message: "[1]直接搜书\n[2]书架选书",
            choices: ["1", "2"],
        });

        switch (aAnswer.option) {
            case "1":
                const bookNameAnswer = await inquirer.prompt({
                    type: "input",
                    name: "bookName",
                    message: "请输入书名：",
                });
                const books = await this.client.searchInfos(bookNameAnswer.bookName);
                const id = books ? await this.selectBookFromList(books) : books;
                id && this.client.volumeInfos(id);
                break;
            case "2":
                //...
                break;
        }
    }

    async Account() {
        const answer = await inquirer.prompt({
            type: "list",
            name: "option",
            message: "[1]添加账号\n[2]删除账号\n选择一个选项:",
            choices: ["1", "2"],
        });
        switch (answer.option) {
            case "1":
                const userNameAnswer = await inquirer.prompt({
                    type: "input",
                    name: "userName",
                    message: "输入账号：",
                });
                const passWordAnswer = await inquirer.prompt({
                    type: "password",
                    name: "passWord",
                    message: "输入密码：",
                    mask: "*",
                });
                const acconutInfo = await this.getUserInfo(
                    userNameAnswer.userName,
                    passWordAnswer.passWord
                );
                this.accountManager.addAccount(acconutInfo as IaccountInfo);
                break;
            case "2":
                //...
                break;
        }
    }

    Regist() { }

    Bonus() { }

    Multi() { }

    async downLoadBook(bookId: number) {
        const volunms = await this.client.volumeInfos(bookId);
    }

    async selectBookFromList(books: IsearchInfos[]): Promise<number> {
        
        const choices = books.map((book, index) => {
            return {
                name: `[${index + 1}] ${book.novelName} - ${book.authorId}`,
                value: index,
            };
        });

        const answer = await inquirer.prompt({
            type: "list",
            name: "bookIndex",
            message: "请输入书序号选择：",
            choices: choices,
        });

        return books[answer.bookIndex].novelId;
    }

    async getUserInfo(userName: string, passWord: string) {
        await this.client.login(userName, passWord);
        const baseinfo = await this.client.userInfo();
        const money = await this.client.userMoney();
        const cookie: string = this.client.cookieJar.getCookieStringSync(
            SfacgHttp.HOST
        );
        const acconutInfo = {
            userName: userName,
            passWord: passWord,
            cookie: cookie,
            ...baseinfo,
            ...money,
        };
        return acconutInfo;
    }
}
