import { sms } from "../../utils/sms";
import { SfacgHttp } from "./http";
import { codeverify, nameAvalible, regist, sendCode } from "./types/Types";

export class SfacgRegister extends SfacgHttp {
  phone: number = 0;
  sms: sms;
  constructor() {
    super();
    this.sms = new sms();
  }

  async avalibleNmae(name: string): Promise<boolean> {
    const res = await this.post<nameAvalible>("/users/availablename", {
      nickName: name,
    });
    return res.data.nickName.valid;
  }
  async sendCode() {
    this.phone = await this.sms.sms(50896);
    let res = await this.post<sendCode>(`/sms/${this.phone}/86`, "");
    return res.status.httpCode == 201;
  }

  async codeverify(phoneNum: number, smsAuthCode: number) {
    return this.put<codeverify>(`/sms/${phoneNum}/86`, {
      smsAuthCode: smsAuthCode,
    });
  }

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
    console.log("注册成功，账号ID为" + accountID);
    return accountID;
  }
}
