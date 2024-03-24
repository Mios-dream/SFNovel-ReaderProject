import { sms } from "../../utils/sms";
import { SfacgHttp } from "./basehttp";
import { nameAvalible, sendCode, codeverify, regist } from "./types/Types";

export class SfacgRegister extends SfacgHttp {
    phone: number = 0;
    sms: sms;
    constructor() {
        super();
        this.sms = new sms();
    }

    // 名称可用性检测
    async avalibleNmae(name: string): Promise<boolean> {
        const res = await this.post<nameAvalible>("/users/availablename", {
            nickName: name,
        });
        return res.data.nickName.valid;
    }
    // 发出验证码
    async sendCode() {
        this.phone = await this.sms.sms(50896);
        const res = await this.post<sendCode>(`/sms/${this.phone}/86`, "");
        return res.status.httpCode == 201;
    }
    // 携带验证
    async codeverify(phoneNum: number, smsAuthCode: number) {
        const res = await this.put<codeverify>(`/sms/${phoneNum}/86`, {
            smsAuthCode: smsAuthCode,
        });
        return res.status.httpCode == 200
    }

    // 注册！
    async regist(
        passWord: string,
        nickName: string,
        phoneNum: number,
        smsAuthCode: number
    ) {
        let res = await this.post<regist>("/user", {
            passWord: passWord,
            nickName: nickName,
            countryCode: "86",
            phoneNum: phoneNum,
            email: "",
            smsAuthCode: smsAuthCode,
            shuMeiId: "",
        });
        let accountID = res.data.accountId;
        return accountID;
    }
}
