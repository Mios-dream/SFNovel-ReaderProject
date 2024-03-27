import { Secret, colorize } from "../../../utils/tools";
import { SfacgClient } from "../api/client";
import { IadBonusNum } from "../types/ITypes";
import { tasks } from "../types/Types";

export class Tasker {
    private anonClient: SfacgClient; // 假设的客户端类型
    private success: string[] = [] // 成功日志
    private failed: string[] = []  // 失败日志

    constructor(anonClient: SfacgClient) {
        this.anonClient = anonClient;
    }

    async TaskAll(result: any, userName: string, accountID: number) {
        await this.claimTasks(result)
        await this.performRituals(accountID)
        await this.checkAndClaimRewards()
        await this.handleAdRewards()
        await this.BonusLog(userName)
    }

    // 接受任务
    async claimTasks(result: any) {
        result.map(async (task: tasks) => {
            task.status == 0 && await this.anonClient.claimTask(task.taskId)
        })
    }

    // 祖传三件套
    async performRituals(accountID: number) {
        await this.anonClient.readTime(120)// 阅读时长
        await this.anonClient.share(accountID) // 每日分享
        await this.anonClient.androiddeviceinfos(accountID)// 上报设备信息
        const signInfo = await this.anonClient.newSign() // 签到
        signInfo ? this.success.push("签到成功") : this.failed.push("签到失败")
    }

    // 任务领取奖励
    async checkAndClaimRewards() {
        const PendingRewards = await this.anonClient.getTasks() // 查看已做待领取任务
        PendingRewards && PendingRewards.map(async (task: tasks) => {
            task.status == 1 && task.name !== "每日签到" && await this.anonClient.taskBonus(task.taskId)
        })
        const logTask = await this.anonClient.getTasks()
        logTask && logTask.map((task: tasks) => {
            task.name !== "每日签到" && task.status === 2 ? this.success.push(task.name + "成功") : this.failed.push(task.name + "失败")
        })

    }


    // 广告任务奖励
    async handleAdRewards() {
        let adWacthtime: number = 0
        const { taskId, requireNum, completeNum } = await this.anonClient.adBonusNum() as IadBonusNum // 广告基础信息
        await this.anonClient.claimTask(taskId)
        for (let i = 0; i < requireNum - completeNum; i++) {
            const adBonusInfo = await this.anonClient.adBonus(taskId)// 广告奖励
            await this.anonClient.taskBonus(taskId)// 这个不知道有没有用
            adBonusInfo && adWacthtime++
        }
        adWacthtime && this.success.push(`广告成功观看了${adWacthtime}次`)
        adWacthtime != 5 && this.failed.push(`广告失败观看了${adWacthtime}次`)

    }

    async BonusLog(userName: string) {
        let a = colorize(`用户${Secret(userName)}的任务日志：`, "blue")
        let b = colorize(`${this.success.join("\n")}`, "green")
        let c = colorize(`${this.failed.join("\n")}`, "yellow")
        console.log(a + "\n" + b + "\n" + c + "\n");
    }

}