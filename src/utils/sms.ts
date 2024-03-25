import axios from "axios";
import { smsGetPhone, smsLogin } from "./types/types";


export enum sid {
  Sfacg = 50896,
  Ciweimao = 22439
}
export enum smsAction {
  cacel = "cancelRecv",
  get = "getPhone"
}

const headers = {
  "Accept": "*/*",
  "Accept-Language": "zh-CN,zh;q=0.9",
  "Cache-Control": "no-cache",
  "Content-Type": "application/x-www-form-urlencoded; charset=UTF-8",
  "Origin": "http://h5.haozhuma.com",
  "Pragma": "no-cache",
  "Proxy-Connection": "keep-alive",
  "Referer": "http://h5.haozhuma.com/login.php",
  "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36 Edg/122.0.0.0",
  "X-Requested-With": "XMLHttpRequest"
}



export class sms {
  private userName: string;
  private passWord: string;
  token: any

  constructor() {
    this.userName = process.env.SMS_USERNAME ?? ""
    this.passWord = process.env.SMS_PASSWORD ?? ""
    if (!this.userName || !this.passWord) {
      console.error("无接码账号信息，请先填写！")
      process.exit()
    }
  }



  // 登录
  async login(retries = 3) {
    while (retries > 0) {
      try {
        const res = await axios.post<smsLogin>("http://h5.haozhuma.com/login.php", {
          username: this.userName,
          password: this.passWord,
        }, { headers, timeout: 5000 });
        this.token = res.data.token;
        console.log("登录成功！");
        return this.token
      } catch (err: any) {
        if (err.code === "ECONNABORTED") {
          retries--;
          console.log("登录超时，重新登录");
          await this.login(retries)
        } else {
          console.log(err.response.data)
          break
        }
      }
    }
  }

  async getPhone(sid: sid, api: smsAction = smsAction.get) {
    try {
      const res = await axios.post<smsGetPhone>("http://api.haozhuma.com/sms/", {
        api: api,
        token: this.token,
        sid: sid,
        Province: "",
        ascription: ""
      }, { headers })
      console.log("获取号码成功！");
      return res.data.phone
    } catch (err: any) {
      return false
    }

  }
  async receive(sid: sid, phone: string): Promise<number | false> {
    try {
      const res = await axios.get("http://api.haozhuma.com/sms", {
        headers, params: {
          api: "getMessage",
          token: this.token,
          sid: sid,
          phone: phone,
          tm: new Date().getTime()
        }
      })
      return res.data.yzm
    }
    catch (err: any) {
      console.log(err.response.data)
      return false
    }
  }
}
