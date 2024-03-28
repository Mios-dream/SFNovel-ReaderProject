import { sid, sms, smsAction } from "../../../utils/sms";
import { RandomName } from "../../../utils/tools";
import { SfacgRegist } from "../api/regist";
import { IaccountInfo } from "../types/ITypes";
import { _SfacgCache } from "./cache";


export class _SfacgRegister {
    regist: SfacgRegist;
    sms: sms;
    constructor() {
        this.regist = new SfacgRegist();
        this.sms = new sms()
    }

    /**
     * 静态方法供外部调用
     */
    static async Register() {
        const register = new _SfacgRegister()
        await register.register()
    }


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
    private async GetAvalibleName(): Promise<string> {
        const name = RandomName()
        const res = await this.regist.avalibleNmae(name);
        console.log(`获取到的昵称：${name}`);
        return res ? name : await this.GetAvalibleName()
    }

    private async GetAvaliblePhone(): Promise<string> {
        await this.sms.login()
        const phone = await this.sms.getPhone(sid.Sfacg)
        console.log(`获取到的手机号：${phone}`);
        const status = phone && this.regist.sendCode(phone)
        !status && this.sms.getPhone(sid.Sfacg, smsAction.cancel)
        return status ? phone : await this.GetAvaliblePhone()
    }
}

