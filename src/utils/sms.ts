import axios from "axios";
import Config from "./config";
import fs from "fs";

export class sms {
  private userName: string;
  private passWord: string;
  private token: string;

  constructor() {
    this.userName = Config.Register.userName;
    this.passWord = Config.Register.passWord;
    this.token = Config.Register.token;
  }

  // 异步的初始化方法
  async init() {
    if (!this.token) {
      if (this.userName && this.passWord) {
        this.token = await this.login();
      } else {
        console.error("no RegisterInfo, YOU CANT USE IT");
      }
    }
  }

  async login(): Promise<string> {
    try {
      const response = await axios.post("http://h5.haozhuma.com/login.php", {
        username: this.userName,
        password: this.passWord,
      });
      // 假设登录接口返回的token在response.data.token中
      return response.data.token;
    } catch (err) {
      // 处理错误
      console.error("Login failed:", err);
      throw err;
    }
  }
}
