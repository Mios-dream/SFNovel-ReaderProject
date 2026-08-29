
import { SfacgHttp } from "./basehttp";
import { nameAvalible, sendCode, codeverify, regist } from "../types/Types";

export class SfacgRegist extends SfacgHttp {

    /**
     * 检查昵称是否可用。
     * @param name 待检查的昵称。
     * @returns 昵称可用时返回 true。
     */
    async avalibleNmae(name: string): Promise<boolean> {
        try {
            const res = await this.post<nameAvalible>("/users/availablename", {
                nickName: name,
            });
            return res.data.nickName.valid;
        } catch (err: any) {
            const errMsg = err.response.data.status.msg
            console.error(
                `POST avalibleNmae failed: ${JSON.stringify(errMsg)}`
            );
            return false;
        }
    }

    /**
     * 向手机号发送注册验证码。
     * @param phone 待验证的手机号。
     * @returns 验证码发送成功时返回 true。
     */
    async sendCode(phone: string) {
        try {
            const res = await this.post<sendCode>(`/sms/${phone}/86`, "");
            return res.status.httpCode == 201;
        } catch (err: any) {
            const errMsg = err.response.data.status.msg
            console.error(
                `POST sendCode failed: ${JSON.stringify(errMsg)}`
            );
            return false;
        }

    }

    /**
     * 向 SF 提交短信验证码进行校验。
     * @param phone 已接收验证码的手机号。
     * @param smsAuthCode 用户收到的短信验证码。
     * @returns 验证成功时返回 true。
     */
    async codeverify(phone: string, smsAuthCode: number) {
        try {
            const res = await this.put<codeverify>(`/sms/${phone}/86`, {
                smsAuthCode: smsAuthCode,
            });
            return res.status.httpCode == 200
        } catch (err: any) {
            const errMsg = err.response.data.status.msg
            console.error(
                `PUT codeverify failed: ${JSON.stringify(errMsg)}`
            );
            return false;
        }

    }

    /**
     * 使用手机号、昵称和验证码创建 SF 账号。
     * @param passWord 新账号密码。
     * @param nickName 新账号昵称。
     * @param phone 已验证的手机号。
     * @param smsAuthCode 已通过校验的短信验证码。
     * @returns 新账号编号；注册失败时返回 false。
     */
    async regist(
        passWord: string,
        nickName: string,
        phone: string,
        smsAuthCode: number
    ) {
        try {
            let res = await this.post<regist>("/user", {
                passWord: passWord,
                nickName: nickName,
                countryCode: "86",
                phoneNum: phone,
                email: "",
                smsAuthCode: smsAuthCode,
                shuMeiId: "",
            });
            let accountID = res.data.accountId;
            return accountID;
        } catch (err: any) {
            const errMsg = err.response.data.status.msg
            console.error(
                `POST regist failed: ${JSON.stringify(errMsg)}`
            );
            return false;
        }

    }
}
