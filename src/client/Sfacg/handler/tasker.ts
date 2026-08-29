import { Secret, colorize } from "../../utils//tools";
import { SfacgClient } from "../api/client";
import { IadBonusNum } from "../types/ITypes";
import { tasks } from "../types/Types";
import { _SfacgCache } from "./cache";

export class _SfacgTasker {
    private Client: SfacgClient; // 假设的客户端类型
    private success: string[] = [] // 成功日志
    private failed: string[] = []  // 失败日志

    /**
     * 创建任务执行器。
     * @param Client 已初始化的 SF API 客户端。
     */
    constructor(Client: SfacgClient) {
        this.Client = Client;
    }
    /**
     * 遍历账号并执行领取任务、签到、广告和奖励流程。
     * @returns 全部账号任务调度完成后的 Promise。
     */
    static async TaskAll() {
        const accounts = await _SfacgCache.GetallCookies()
        accounts?.map(async (account) => {
            const { result, anonClient } = await SfacgClient.initClient(account, "getTasks")// 初始化客户端
            const tasker = new _SfacgTasker(anonClient)
            await tasker.claimTasks(result)
            await tasker.performRituals(account.accountId)
            await tasker.checkAndClaimRewards()
            await tasker.handleAdRewards()
            await tasker.BonusLog(account.userName)
            // 更新账号
            account.cookie = anonClient.GetCookie()
            await _SfacgCache.UpdateAccount(account)
        })
    }

    /**
     * 领取任务列表中尚未领取的任务。
     * @param result SF 任务接口返回的任务数组。
     * @returns 任务领取调度完成后的 Promise。
     */
    async claimTasks(result: any) {
        result.map(async (task: tasks) => {
            task.status == 0 && await this.Client.claimTask(task.taskId)
        })
    }

    /**
     * 执行阅读、分享、设备上报和签到组合流程，失败时重试。
     * @param accountID SF 账号编号。
     * @param retry 剩余重试次数，默认 3 次。
     * @returns 组合任务完成后的 Promise。
     */
    async performRituals(accountID: number, retry = 3) {
        if (retry > 0) {
            await this.Client.readTime(120)// 阅读时长
            await this.Client.share(accountID) // 每日分享
            await this.Client.androiddeviceinfos(accountID)// 上报设备信息
            const signInfo = await this.Client.newSign() // 签到 
            retry--
            signInfo ? this.success.push("签到成功") : await this.performRituals(accountID, retry)
        }
    }

    /**
     * 领取已完成任务奖励并记录每项任务结果。
     * @returns 奖励检查完成后的 Promise。
     */
    async checkAndClaimRewards() {
        const PendingRewards = await this.Client.getTasks() // 查看已做待领取任务
        PendingRewards && PendingRewards.map(async (task: tasks) => {
            task.status == 1 && task.name !== "每日签到" && await this.Client.taskBonus(task.taskId)
        })
        const logTask = await this.Client.getTasks()
        logTask && logTask.map((task: tasks) => {
            task.name !== "每日签到" && (task.status === 2 ? this.success.push(task.name + "成功") : this.failed.push(task.name + "失败"))
        })
    }

    /**
     * 完成剩余广告观看次数并领取广告任务奖励。
     * @returns 广告奖励流程完成后的 Promise。
     */
    async handleAdRewards() {
        let adWacthtime: number = 0
        const { taskId, requireNum, completeNum } = await this.Client.adBonusNum() as IadBonusNum // 广告基础信息
        await this.Client.claimTask(taskId)
        for (let i = 0; i < requireNum - completeNum; i++) {
            const adBonusInfo = await this.Client.adBonus(taskId)// 广告奖励
            await this.Client.taskBonus(taskId)// 这个不知道有没有用
            adBonusInfo && adWacthtime++
        }
        this.success.push(`广告成功观看了${adWacthtime}次`)
        adWacthtime != 5 && this.failed.push(`广告失败观看了${5 - adWacthtime}次`)
    }

    /**
     * 输出指定账号的任务成功和失败日志。
     * @param userName 要显示的账号名。
     * @returns 无返回值。
     */
    async BonusLog(userName: string) {
        let a = colorize(`用户${Secret(userName)}的任务日志：`, "blue")
        let b = colorize(`${this.success.join("\n")}`, "green")
        let c = colorize(`${this.failed.join("\n")}`, "yellow")
        console.log(a + "\n" + b + "\n" + c + "\n");
    }

}
// (async () => {
//     await _SfacgTasker.TaskAll()
// })()
