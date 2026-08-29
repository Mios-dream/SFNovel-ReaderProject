import { sid, sms, smsAction } from "../../utils//sms";
import { RandomName } from "../../utils//tools";
import { SfacgRegist } from "../api/regist";
import { IaccountInfo } from "../types/ITypes";
import { _SfacgCache } from "./cache";


export class _SfacgRegister {
    regist: SfacgRegist;
    sms: sms;
    /** 创建注册流程所需的注册 API 和短信服务客户端。 */
    constructor() {
        this.regist = new SfacgRegist();
        this.sms = new sms()
    }

    /**
     * 启动一次完整的自动注册流程。
     * @returns 注册流程完成后的 Promise。
     */
    static async Register() {
        const register = new _SfacgRegister()
        await register.register()
    }


    /**
     * 获取可用手机号、昵称并完成短信验证和账号创建。
     * @returns 注册流程完成后的 Promise。
     */
    async register() {
        const phone = await this.GetAvaliblePhone()// 这里已经同时发送短信了，不必重复操作
        const name = await this.GetAvalibleName()
        console.log(`获取到的昵称：${name}，获取到的手机号：${phone}`);
        if (phone) {
            const code = await this.sms.waitForCode(sid.Sfacg, phone)
            const verify = code && await this.regist.codeverify(phone, code)
            const AcountId = verify && await this.regist.regist(process.env.REGIST_PASSWORD ?? "dddd1111", name, phone, code)
            if (AcountId) {
                console.log(`注册成功，账号：${phone}，密码：${process.env.REGIST_PASSWORD ?? "dddd1111"}`);
                await _SfacgCache.UpdateAccount({ userName: phone, passWord: process.env.REGIST_PASSWORD ?? "dddd1111" } as IaccountInfo, true)
            }
        }
    }
    /**
     * 递归生成并检查可用昵称。
     * @returns 已通过 SF 检查的昵称。
     */
    private async GetAvalibleName(): Promise<string> {
        const name = RandomName()
        const res = await this.regist.avalibleNmae(name);
        console.log(`获取到的昵称：${name}`);
        return res ? name : await this.GetAvalibleName()
    }

    /**
     * 从短信服务获取手机号并触发 SF 验证码发送。
     * @returns 可用手机号；获取失败时返回空字符串。
     */
    private async GetAvaliblePhone(): Promise<string> {
        await this.sms.login()
        const phone = await this.sms.getPhone(sid.Sfacg)
        console.log(`获取到的手机号：${phone}`);
        const status = phone && this.regist.sendCode(phone)
        !status && this.sms.getPhone(sid.Sfacg, smsAction.cancel)
        return phone ? phone : ""
    }
}

