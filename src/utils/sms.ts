import axios from "axios";
import Config from "./config";



export class sms {
  private userName: string;
  private passWord: string;
  private token: string

  constructor() {
    this.userName = Config.Register.userName;
    this.passWord = Config.Register.passWord;
    this.token = Config.Register.token
  }

  async sms(app: string) {
    let sid: number = 50896
    switch (app) {
      case "sfacg":
        sid = 50896
      case "ciweimao":
        sid = 22439
    }
    await this.getPhone(sid)
  }
  private async login() {
    const res = await axios.post("http://h5.haozhuma.com/login.php", {
      username: this.userName,
      password: this.passWord,
    });
    this.token = res.data.token;
    Config.Register.token = this.token
  }
  private async getPhone(sid: number): Promise<number> {
    const res = await axios.post("http://api.haozhuma.com/sms/", {
      api: "getPhone",
      token: this.token,
      sid: sid,
      Province: "",
      ascription: ""
    })
    return res.data.phone
  }
  private async receive(sid: number, phone: number): Promise<any> {
    const res = await axios.post("http://api.haozhuma.com/sms", {
      api: "getMessage",
      token: this.token,
      sid: sid,
      phone: phone,
      tm: Date
    })
    return res.data
  }
}
