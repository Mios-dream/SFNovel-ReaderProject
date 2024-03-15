import axios from "axios";
import Config from "./config";
import { smsGetPhone, smsLogin } from "./types/types";


export class sms {
  private userName: string;
  private passWord: string;
  private token: string

  constructor() {
    this.userName = Config.Register.userName;
    this.passWord = Config.Register.passWord;
    this.token = Config.Register.token
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
    Config.Register.token = this.token
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
