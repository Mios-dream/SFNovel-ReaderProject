import { colorize, question, questionAccount } from "../../utils/tools";
import { _SfacgTasker } from "./handler/tasker";
import { _SfacgCache } from "./handler/cache";
import { _SfacgRegister } from "./handler/register";
import { _SfacgDownloader } from "./handler/download";

export class Sfacg {
    async init() {
        console.log("选择一个选项:");
        console.log(colorize("1. 帮人提书", "blue"));
        console.log(colorize("2. 每日奖励", "blue"));
        console.log(colorize("3. 账号管理", "blue"));
        console.log(colorize("4. 多账号提书", "blue"));
        console.log(colorize("5. 注册机启动！", "blue"));
        const option = await question(colorize("请输入选项的数字："
            , "green"));
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
                console.log(colorize("输入的选项不正确，请重新输入。"
                    , "yellow"));
                this.init();
                break;
        }
    }

    async Once() {
        await _SfacgDownloader.Once();
    }

    async Account() {
        console.log(colorize("[1]添加账号"
            , "blue"));
        console.log(colorize("[2]删除账号"
            , "blue"));
        const option = await question(colorize("选择一个选项:"
            , "yellow"));
        switch (option) {
            case "1":
                const { userName, passWord } = await questionAccount();
                await _SfacgCache.UpdateAccount({
                    userName: userName as string,
                    passWord: passWord as string,
                });
                break;
            case "2":
                const a = await question(colorize("输入账号："
                    , "blue"));
                await _SfacgCache.RemoveAccount(a as string);
                break;
            default:
                console.log(colorize("输入的选项不正确。"
                    , "blue"));
                await this.Account();
                break;
        }
    }

    async Regist() {
        await _SfacgRegister.Register();
    }

    async Bonus() {
        await _SfacgTasker.TaskAll();
    }

    async Multi() { }
}

(async () => {
    const a = new Sfacg();
    await a.init();
})();
