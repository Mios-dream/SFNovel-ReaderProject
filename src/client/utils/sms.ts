import axios from "axios";
import { smsGetPhone, smsLogin } from "./types/types";


export enum sid {
  Sfacg = 50896,
  Ciweimao = 22439
}
export enum smsAction {
  cancel = "cancelRecv",
  get = "getPhone"
}

const headers = {
  "Accept": "*/*",
  "Accept-Language": "zh-CN,zh;q=0.9",
  "Cache-Control": "no-cache",
  "Content-Type": "application/x-www-form-urlencoded; charset=UTF-8",
  "Origin": "https://h5.haozhuma.com",
  "Pragma": "no-cache",
  "Proxy-Connection": "keep-alive",
  "Referer": "https://h5.haozhuma.com/login.php",
  "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36 Edg/122.0.0.0",
  "X-Requested-With": "XMLHttpRequest"
}



export class sms {
  private userName: string;
  private passWord: string;
  token: any

  /** 从环境变量读取接码平台凭据并初始化客户端。 */
  constructor() {
    this.userName = process.env.SMS_USERNAME ?? ""
    this.passWord = process.env.SMS_PASSWORD ?? ""
    if (!this.userName || !this.passWord) {
      console.error("无接码账号信息，请先填写！")
      process.exit()
    }
  }

  /**
   * 轮询接码平台直到收到短信验证码。
   * @param sid SF 或其他平台的服务编号。
   * @param phone 待接收验证码的手机号。
   * @returns 收到的数字验证码。
   */
  async waitForCode(sid: sid, phone: string): Promise<number> {
    return new Promise(resolve => {
      const checkCode = async () => {
        const code = await this.receive(sid, phone);
        if (code) {
          resolve(code); // 完成Promise，并返回code值
        } else {
          // 如果code为空，5秒后再次检查
          setTimeout(checkCode, 5000);
        }
      };
      checkCode();
    });
  }

  /**
   * 登录接码平台并保存访问令牌。
   * @param retries 网络超时时的剩余重试次数，默认 3 次。
   * @returns 接码平台令牌；登录失败时返回 undefined。
   */
  async login(retries = 3) {
    if (retries > 0) {
      try {
        const res = await axios.post<smsLogin>("https://h5.haozhuma.com/login.php", {
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
        }
      }
    }
  }

  /**
   * 获取或释放一个接码平台手机号。
   * @param sid 目标业务服务编号。
   * @param api 操作类型，默认获取手机号。
   * @returns 获取成功时返回手机号，失败时返回 false。
   */
  async getPhone(sid: sid, api: smsAction = smsAction.get) {
    try {
      const res = await axios.post<smsGetPhone>("https://api.haozhuma.com/sms/", {
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
  /**
   * 查询手机号收到的最新短信验证码。
   * @param sid 目标业务服务编号。
   * @param phone 待查询的手机号。
   * @returns 验证码或 false（尚未收到/请求失败）。
   */
  private async receive(sid: sid, phone: string): Promise<number | false> {
    try {
      const res = await axios.get("https://api.haozhuma.com/sms", {
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
