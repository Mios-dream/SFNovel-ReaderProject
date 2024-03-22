import axios from "axios";
import { smsGetPhone, smsLogin } from "./types/types";


export class sms {
  private userName: string;
  private passWord: string;
  private token: any

  constructor() {
    this.userName = process.env.SMS_USERNAME ?? "1"
    this.passWord = process.env.SMS_PASSWORD ?? "1"
  }

  async sms(sid: number) {
    let phone = await this.getPhone(sid)
    if (!this.token || phone == 0) {
      await this.login()
      phone = await this.getPhone(sid)
    }
    return phone
  }

  private async login() {
    const res = await axios.post<smsLogin>("http://h5.haozhuma.com/login.php", {
      username: this.userName,
      password: this.passWord,
    });
    this.token = res.data.token;
  }
  private async getPhone(sid: number): Promise<number> {
    try {
      const res = await axios.post<smsGetPhone>("http://api.haozhuma.com/sms/", {
        api: "getPhone",
        token: this.token,
        sid: sid,
        Province: "",
        ascription: ""
      })
      return Number(res.data.phone)
    } catch (err: any) {
      return 0
    }

  }
  async receive(sid: number, phone: number): Promise<any> {
    const res = await axios.post("http://api.haozhuma.com/sms", {
      api: "getMessage",
      token: this.token,
      sid: sid,
      phone: phone,
      tm: new Date().getTime
    })
    return res.data
  }
}
