import { IaccountInfo, IadBonusNum, IsearchInfos } from "./types/ITypes";
import { SfacgCache } from "./cache";
import { SfacgClient } from "./client";
import readline from "readline"
import { SfacgRegister } from "./register";
import { sid, sms } from "../../utils/sms";
import { tasks } from "./types/Types";
import { promises } from "fs-extra";

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
    SfacgCache: SfacgCache;
    register: SfacgRegister;
    sms: sms;

    constructor() {
        this.client = new SfacgClient();
        this.SfacgCache = new SfacgCache();
        this.register = new SfacgRegister();
        this.sms = new sms();
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
                const bookName = await question("请输入书名：");
                const books = await this.client.searchInfos(bookName as string);
                const id = books ? await this.selectBookFromList(books) : null;
                if (id) {
                    await this.client.volumeInfos(id);
                }
                break;
            case "2":
                // .. ...
                break;
            default:
                console.log("输入的选项不正确。");
                await this.Once();
                break;
        }

        rl.close();
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
                this.SfacgCache.removeAccount(a as string)
                break;
            default:
                console.log("输入的选项不正确。");
                await this.Account();
                break;
        }

        rl.close();
    }

    async Regist() {
        rl.close();
        const phone = await this.GetAvaliblePhone()
        const name = await this.GetAvalibleName()
        console.log(`获取到的昵称：${name}，获取到的手机号：${phone}`);
        const _send = await this.register.sendCode(phone)
        if (_send) {
            const code = await this.waitForCode(sid.Sfacg, phone)
            const verify = code && await this.register.codeverify(phone, code)
            const AcountId = verify && await this.register.regist(process.env.REGIST_PASSWORD ?? "dddd1111", name, phone, code)
            if (AcountId) {
                console.log(`注册成功，账号：${phone}，密码：${process.env.REGIST_PASSWORD ?? "dddd1111"}`);
                await this.updateUserInfo({ userName: phone, passWord: process.env.REGIST_PASSWORD ?? "dddd1111" } as IaccountInfo)
            }
        }
    }

    async Bonus() {
        rl.close();
        const accounts = await this.SfacgCache.allCookiesGet()
        accounts?.map(async (account) => {
            const { result, anonClient } = await this.initClient(account, "getTasks")// 初始化客户端，判断ck是否有效，返回可用线程
            account.cookie = anonClient.cookie
            console.log((result as tasks[]).map((task) => task.name))
            result.map(async (task: tasks) => {
                if (task.status == 0)
                    await anonClient.claimTask(task.taskId)
            })
            const signInfo = await anonClient.newSign() // 签到
            signInfo && console.log(`用户${account.userName}签到成功`);
            await anonClient.readTime(120)// 阅读时长
            await anonClient.share(account.accountId) // 每日分享
            const PendingRewards = await anonClient.getTasks() // 查看已做待领取任务
            PendingRewards && PendingRewards.map(async (task: tasks) => {
                if (task.status == 1 && task.name !== "每日签到") {
                    const staus = await anonClient.taskBonus(task.taskId)
                    staus && console.log(`任务${task.name}已完成`);
                }
            })
            const { taskId, requireNum, completeNum } = await anonClient.adBonusNum() as IadBonusNum // 广告基础信息
            await anonClient.claimTask(taskId) // 接受广告任务
            console.log(`需要观看广告的次数：${requireNum - completeNum} `)
            for (let i = 0; i < requireNum - completeNum; i++) {
                const adBonusInfo = await anonClient.adBonus(taskId)// 广告奖励
                await anonClient.taskBonus(taskId)// 这个不知道有没有用
                adBonusInfo && console.log(`用户${account.userName}完成了第${i}次广告`);
            }
            await this.updateUserInfo(account) // 账号信息更新
        })

    }

    async Multi() {


    }

    async downLoadBook(bookId: number) {
        const volunms = await this.client.volumeInfos(bookId);
        volunms && volunms
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


    // 更新用户账号信息
    async updateUserInfo(acconutInfo: IaccountInfo, newAccount: boolean = false) {
        const { userName, passWord } = acconutInfo
        const { result, anonClient } = await this.initClient(acconutInfo, "userInfo")
        if (newAccount) {
            const Fav = await anonClient.NewAccountFavBonus()
            Fav && console.log("新号收藏任务完成")
            const Follow = await anonClient.NewAccountFollowBonus()
            Follow && console.log("新号关注任务完成")
        }
        const money = await anonClient.userMoney()
        const accountInfo = {
            userName: userName,
            passWord: passWord,
            cookie: anonClient.cookie,
            ...result,
            ...money,
        };
        (result && money) ? this.SfacgCache.UpsertAccount(accountInfo as IaccountInfo) : console.log("账号信息获取失败，请检查账号密码")

    }

    // 接收账号信息和要做的，测试ck可用性，返回函数的返回内容和可用的线程
    async initClient(acconutInfo: IaccountInfo, todo: "getTasks" | "userInfo") {
        // console.log("进入初始线程");
        const anonClient = new SfacgClient()
        const { userName, passWord, cookie } = acconutInfo
        let result: any
        if (cookie) {
            anonClient.cookie = cookie
            result = (todo == "getTasks") ? await anonClient.getTasks() : await anonClient.userInfo()
            result && console.log(`${this.Secret(acconutInfo.userName as string)}原ck可用`);
        }
        if ((!cookie || !result) && userName && passWord) {
            const a = await anonClient.login(userName, passWord)
            if (a) {
                console.log(`${this.Secret(acconutInfo.userName as string)}ck重置`);
                result = (todo == "getTasks") ? await anonClient.getTasks() : await anonClient.userInfo()
            }
            else {
                console.log("重新获取ck失败")
            }
        }
        return { result, anonClient }
    }

    private Secret(phoneNumber: string) {
        return phoneNumber.replace(/(\d{3})\d{4}(\d{4})/, '$1****$2');
    }

    private RandomName() {
        // 生成随机六位汉字+字母+数字组合的代码
        const random = (min: number, max: number) => Math.floor(Math.random() * (max - min) + min);
        const randomChar = (length: number) => {
            const chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
            let result = '';
            for (let i = 0; i < length; i++) {
                result += chars[random(0, chars.length)];
            }
            return result;
        };
        const randomChinese = (length: number) => {
            let result = '';
            for (let i = 0; i < length; i++) {
                result += String.fromCharCode(random(0x4e00, 0x9fa5));
            }
            return result;
        };
        const randomName = randomChinese(2) + randomChar(4);
        return randomName;
    }

    private async GetAvalibleName(): Promise<string> {
        const name = this.RandomName()
        const res = await this.register.avalibleNmae(name);
        console.log(`获取到的昵称：${name}`);
        return res ? name : await this.GetAvalibleName()
    }

    private async GetAvaliblePhone(): Promise<string> {
        await this.sms.login()
        const phone = await this.sms.getPhone(sid.Sfacg)
        console.log(`获取到的手机号：${phone}`);
        const status = phone && this.register.sendCode(phone)
        return status ? phone : await this.GetAvaliblePhone()
    }


    async waitForCode(sid: sid, phone: string): Promise<number> {
        return new Promise(resolve => {
            const checkCode = async () => {
                const code = await this.sms.receive(sid, phone);
                if (code) {
                    resolve(code); // 完成Promise，并返回code值
                } else {
                    // 如果code为空，5秒后再次检查
                    setTimeout(checkCode, 5000);
                }
            };
            checkCode();
        });
    }
}



