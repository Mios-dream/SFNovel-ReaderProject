import { SfacgHttp } from "./http";

export class SfacgRegister extends SfacgHttp {
  // 验证名称是否可用
  /**
    成功响应
    {"status":{"httpCode":200,"errorCode":200,"msgType":0,"msg":null},"data":{"availableName":"hehdvs3","nickName":{"valid":true,"msg":"success"}}}

    失败响应（其一）
    {"status":{"httpCode":200,"errorCode":200,"msgType":0,"msg":null},"data":{"availableName":"hehdvs3","nickName":{"valid":false,"msg":"该昵称存在标点符号，请修改"}}}
     * @param name 
     * @returns 
     */
  async avalibleNmae(name: string) {
    return await this.post("/users/availablename", {
      nickName: name,
    });
  }
  async sendCode(phoneNum: number) {
    return await this.post(`/sms/${phoneNum}/86`, "");
  }

  async codeverify(phoneNum: number, smsAuthCode: number) {
    return await this.put(`/sms/${phoneNum}/86`, { smsAuthCode: smsAuthCode });
  }

  async regist(
    passWord: string,
    nickName: string,
    phoneNum: number,
    smsAuthCode: number
  ) {
    return await this.post("/user", {
      passWord: passWord,
      nickName: nickName,
      countryCode: "86",
      phoneNum: phoneNum,
      email: "",
      smsAuthCode: smsAuthCode,
      shuMeiId: "",
    });
  }
}
