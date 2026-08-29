import { colorize, question, questionAccount } from "../utils/tools";
import { _SfacgTasker } from "./handler/tasker";
import { _SfacgCache } from "./handler/cache";
import { _SfacgRegister } from "./handler/register";
import { _SfacgDownloader } from "./handler/download";

export class Sfacg {
  /**
   * 显示命令行菜单并分派用户选择的操作。
   * @returns 菜单处理完成后的 Promise。
   */
  async init() {
    console.log("选择一个选项:");
    console.log(colorize("1. 帮人提书", "blue"));
    console.log(colorize("2. 每日奖励", "blue"));
    console.log(colorize("3. 账号管理", "blue"));
    console.log(colorize("4. 多账号提书", "blue"));
    console.log(colorize("5. 注册机启动！", "blue"));
    console.log(colorize("6. 数据库中下载", "blue"));
    const option = await question(colorize("请输入选项的数字：", "green"));
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
      case "6":
        this.ServerDownload();
        break;
      default:
        console.log(colorize("输入的选项不正确，请重新输入。", "yellow"));
        this.init();
        break;
    }
  }

  /**
   * 执行一次交互式书架下载流程。
   * @returns 下载流程完成后的 Promise。
   */
  async Once() {
    await _SfacgDownloader.Once();
  }

  /**
   * 管理本地缓存的 SF 账号。
   * @returns 账号操作完成后的 Promise。
   */
  async Account() {
    console.log(colorize("[1]添加账号", "blue"));
    console.log(colorize("[2]删除账号", "blue"));
    const option = await question(colorize("选择一个选项:", "yellow"));
    switch (option) {
      case "1":
        const { userName, passWord } = await questionAccount();
        await _SfacgCache.UpdateAccount({
          userName: userName as string,
          passWord: passWord as string,
        });
        break;
      case "2":
        const a = await question(colorize("输入账号：", "blue"));
        await _SfacgCache.RemoveAccount(a as string);
        break;
      default:
        console.log(colorize("输入的选项不正确。", "blue"));
        await this.Account();
        break;
    }
  }

  /**
   * 启动自动注册流程。
   * @returns 注册流程完成后的 Promise。
   */
  async Regist() {
    await _SfacgRegister.Register();
  }

  /**
   * 执行全部账号的每日任务。
   * @returns 任务流程完成后的 Promise。
   */
  async Bonus() {
    await _SfacgTasker.TaskAll();
  }

  /**
   * 多账号下载入口预留。
   * @returns 已完成的空 Promise。
   */
  async Multi() {}

  /**
   * 从数据库搜索并下载作品。
   * @returns 搜索下载流程完成后的 Promise。
   */
  async ServerDownload() {
    await _SfacgDownloader.Search();
  }
}

(async () => {
  const a = new Sfacg();
  await a.init();
})();
// 16514636462
// 17142762591
// Opooo1830
