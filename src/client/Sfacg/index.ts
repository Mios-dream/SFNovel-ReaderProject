import { IaccountInfo, IadBonusNum, IsearchInfos } from "./types/ITypes";
import { SfacgAccountManager } from "./account";
import { SfacgClient, SfacgRegister } from "./client";
import { SfacgHttp } from "./basehttp";
import readline from "readline"
import { restoreDefaultPrompts } from "inquirer";

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
});

function question(query: any) {
    return new Promise((resolve) => {
        rl.question(query, (answer) => {
            resolve(answer);
        });
    });
}
export class Sfacg {
    client: SfacgClient;
    accountManager: SfacgAccountManager;
    register: SfacgRegister;

    constructor() {
        this.client = new SfacgClient();
        this.accountManager = new SfacgAccountManager();
        this.register = new SfacgRegister();
    }

    async init() {
        console.log("选择一个选项:");
        console.log("1. 帮人提书");
        console.log("2. 每日奖励");
        console.log("3. 账号管理");
        console.log("4. 多账号提书");
        console.log("5. 注册机启动！");
        const option = await question("请输入选项的数字：");
        switch (option) {
            case "1":
                this.Once();
                break;
            case "2":
                this.Bonus();
                break;
            case "3":
                this.Account();
                break;
            case "4":
                this.Multi();
                break;
            case "5":
                this.Regist();
                break;
            default:
                console.log("输入的选项不正确，请重新输入。");
                this.init();
                break;
        }
    }


    async Once() {
        const userName = await question("输入账号：");
        const passWord = await question("输入密码：");
        this.client.login(userName as string, passWord as string);
        console.log("[1]直接搜书");
        console.log("[2]书架选书");
        const option = await question("请选择一个操作：");

        switch (option) {
            case "1":
                // 提问并获取书名
                const bookName = await question("请输入书名：");
                // 调用 searchInfos 方法并处理结果
                const books = await this.client.searchInfos(bookName as string);
                const id = books ? await this.selectBookFromList(books) : null;
                if (id) {
                    await this.client.volumeInfos(id);
                }
                break;
            case "2":
                // ...书架选书的代码
                break;
            default:
                console.log("输入的选项不正确。");
                await this.Once(); // 如果输入错误，重新调用 Once 方法
                break;
        }

        rl.close(); // 关闭 readline 接口
    }

    async Account() {
        console.log("[1]添加账号");
        console.log("[2]删除账号");
        const option = await question("选择一个选项:");

        switch (option) {
            case "1":
                const userName = await question("输入账号：");
                const passWord = await question("输入密码：");
                await this.updateUserInfo({ userName: userName as string, passWord: passWord as string });

                break;
            case "2":
                const a = await question("输入账号：");
                this.accountManager.removeAccount(a as string)
                break;
            default:
                console.log("输入的选项不正确。");
                await this.Account(); // 如果输入错误，重新调用 Account 方法
                break;
        }

        rl.close(); // 关闭 readline 接口
    }

    Regist() { }

    // async Tasker() {
    //   // adBonusNum, newSign, getTasks, claimTask

    //   // adBonus, readTime, share

    //   // taskBonus
    //   await this.newSign()
    //   const tasks = await this.getTasks() as number[]
    //   tasks.map(async (taskId) => {
    //     await this.claimTask(taskId)
    //   })
    //   await this.readTime(120)
    //   await this.share(userId)
    //   tasks.map(async (taskId) => {
    //     await this.taskBonus(taskId)
    //   })
    //   const { taskId, requireNum } = await this.adBonusNum() as IadBonusNum
    //   await this.claimTask(taskId)
    //   console.log(`需要观看广告的次数：${requireNum} `)
    //   for (let i = 0; i < requireNum; i++) {
    //     await this.adBonus(taskId)
    //     await this.taskBonus(taskId)
    //   }
    // }

    async Bonus() {
        const accounts = await this.accountManager.allCookiesGet()
        const failed: string[] = []
        accounts?.map(async (account) => {
            const { result, anonClient } = await this.initClient(account, "getTasks")
            account.cookie = anonClient.cookieJar.getCookieStringSync(SfacgHttp.HOST)
            console.log(result);
            result.map((taskId: any) => {
                anonClient.claimTask(taskId)
            })
            const { taskId, requireNum } = await anonClient.adBonusNum() as IadBonusNum
            const signInfo = await anonClient.newSign()
            signInfo && console.log(`用户${account.userName}签到成功`);
            await anonClient.readTime(120)
            await anonClient.share(account.accountId)
            result.map((taskId: any) => {
                anonClient.taskBonus(taskId)
            })
            await anonClient.claimTask(taskId)
            console.log(`需要观看广告的次数：${requireNum} `)
            for (let i = 0; i < requireNum; i++) {
                const adBonusInfo = await anonClient.adBonus(taskId)
                await anonClient.taskBonus(taskId)
                adBonusInfo && console.log(`用户${account.userName}完成了第${i}次广告`);
            }
            this.updateUserInfo(account)
        })
    }

    Multi() { }

    async downLoadBook(bookId: number) {
        const volunms = await this.client.volumeInfos(bookId);
    }

    async selectBookFromList(books: IsearchInfos[]): Promise<number> {
        // 显示书籍列表
        books.forEach((book, index) => {
            console.log(`[${index + 1}] ${book.novelName} - ${book.authorId}`);
        });
        const bookIndex = await question("请输入书序号选择：");
        const index = bookIndex as number - 1;
        return books[index].novelId;
    }


    async updateUserInfo(acconutInfo: IaccountInfo, firstLogin: boolean = false) {
        const { userName, passWord, cookie } = acconutInfo
        if (firstLogin) {
            await this.client.login(userName as string, passWord as string)
            const baseinfo = await this.client.userInfo();
            const money = await this.client.userMoney();
            const cookie = this.client.cookieJar.getCookieStringSync(SfacgHttp.HOST)
            const accountInfo = {
                userName: userName,
                passWord: passWord,
                cookie: cookie,
                ...baseinfo,
                ...money,
            };
            accountInfo ? this.accountManager.addAccount(accountInfo as IaccountInfo) : console.log("账号信息获取失败，请检查账号密码")
        } else {
            const { result, anonClient } = await this.initClient(acconutInfo, "userInfo")
            const money = await anonClient.userMoney()
            const cookie = anonClient.cookieJar.getCookieStringSync(SfacgHttp.HOST)
            const accountInfo = {
                userName: userName,
                passWord: passWord,
                cookie: cookie,
                ...result,
                ...money,
            };
            accountInfo ? this.accountManager.updateAccount(accountInfo as IaccountInfo) : console.log("账号信息获取失败，请检查账号密码")
        }


    }

    // 接收账号信息和一个失败返回false的函数，测试ck可用性，返回函数的返回内容和可用的线程
    async initClient(acconutInfo: IaccountInfo, todo: "getTasks" | "userInfo") {
        const anonClient = new SfacgClient()
        const { userName, passWord, cookie } = acconutInfo
        let result: any
        if (cookie) {
            console.log("原存ck" + cookie)
            anonClient.cookieJar.setCookieSync(cookie, SfacgHttp.HOST)
            result = (todo == "getTasks") ? await anonClient.getTasks() : await anonClient.userInfo()
            result && console.log(`${acconutInfo.userName}原ck可用`);

        }
        if ((!cookie || !result) && userName && passWord) {
            console.log(`${acconutInfo.userName}ck失效`);
            anonClient.cookieJar.removeAllCookiesSync()
            const a = await anonClient.login(userName, passWord)
            if (a) {
                console.log(`${acconutInfo.userName}ck重置`);
                const newcookie = anonClient.cookieJar.getCookieStringSync(SfacgHttp.HOST)
                console.log("新ck" + newcookie);
                result = (todo == "getTasks") ? await anonClient.getTasks() : await anonClient.userInfo()
            }
            else {
                console.log("重新获取ck失败")
            }
        }
        return { result, anonClient }
    }
}
