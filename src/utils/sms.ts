import axios from "axios";
import Config from "./config";

export class sms {
  private userName: string;
  private passWord: string;
  private token: string = "";

  constructor() {
    this.userName = Config.Register.userName;
    this.passWord = Config.Register.passWord;
  }

  async init() {
    const response = await axios.post("http://h5.haozhuma.com/login.php", {
      username: this.userName,
      password: this.passWord,
    });
    this.token = response.data.token;
  }
}
