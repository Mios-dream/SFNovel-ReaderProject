import axios, { AxiosInstance, AxiosResponse } from "axios";
import { v4 as uuidv4 } from "uuid";
import { wrapper } from "axios-cookiejar-support";
import { CookieJar } from "tough-cookie";
import Config from "../../utils/config";

const crypto = require("crypto");

export class SfacgHttp {
  static readonly HOST = "https://api.sfacg.com";
  static readonly USER_AGENT_PREFIX = "boluobao/4.9.76(iOS;16.6)/appStore/";
  static readonly USER_AGENT_RSS =
    "SFReader/4.9.76 (iPhone; iOS 16.6; Scale/3.00)";
  static readonly USERNAME = "apiuser";
  static readonly PASSWORD = "3s#1-yt6e*Acv@qer";
  static readonly SALT = "FMLxgOdsfxmN!Dt4";
  static readonly DEVICE_TOKEN = uuidv4().toUpperCase();

  private client: AxiosInstance;
  private clientRss: AxiosInstance;
  private cookieJar: CookieJar;

  constructor() {
    this.cookieJar = new CookieJar();
    this.client = this._client();
    this.clientRss = this._clientRss();
  }

  async init() {
    let ProxyUrl = new URL(Config.sfacg.proxy??"");
    if (ProxyUrl) {
      this.client.defaults.proxy = {
        host: ProxyUrl.hostname,
        port: parseInt(ProxyUrl.port),
      };
    }
  }
  async get<T, E = any>(url: string, query?: E): Promise<T> {
    let response: AxiosResponse;
    url.startsWith("/Chaps") ? (this.client = this._client()) : "";
    response = await this.client.get<T>(url, {
      jar: this.cookieJar,
      params: query,
    });
    return url.startsWith("/sessions")
      ? response.data.status
      : response.data.data;
  }

  async get_rss<E>(url: string): Promise<E> {
    let response: AxiosResponse;
    response = await this.clientRss.get(url, {
      jar: this.cookieJar,
    });
    return response.data;
  }

  async post<T, E>(url: string, data: E): Promise<T> {
    let response: any;
    this.client = this._client();
    response = await this.client.post<T>(url, data, {
      jar: this.cookieJar,
    });
    return response.status;
  }

  async put<T, E = any>(url: string, data: E): Promise<T> {
    let response: any;
    this.client = this._client();
    response = await this.client.put<T>(url, data, {
      jar: this.cookieJar,
    });
    return response.data.data;
  }

  getNowFormatDate(): string {
    const date = new Date();
    const year = date.getFullYear();
    const month = (date.getMonth() + 1).toString().padStart(2, "0");
    const day = date.getDate().toString().padStart(2, "0");
    return `${year}-${month}-${day}`;
  }

  private _client() {
    // 初始化axios实例
    return wrapper(
      axios.create({
        withCredentials: true,
        baseURL: SfacgHttp.HOST,
        auth: {
          username: SfacgHttp.USERNAME,
          password: SfacgHttp.PASSWORD,
        },
        headers: {
          Accept: "application/vnd.sfacg.api+json;version=1",
          "Accept-Language": "zh-Hans-CN;q=1",
          "User-Agent": SfacgHttp.USER_AGENT_PREFIX + SfacgHttp.DEVICE_TOKEN,
          SFSecurity: this.sfSecurity(),
        },
      })
    );
  }

  private _clientRss() {
    // 初始化rss client实例
    return (this.clientRss = wrapper(
      axios.create({
        responseType: "arraybuffer",
        baseURL: SfacgHttp.HOST,
        headers: {
          "User-Agent": SfacgHttp.USER_AGENT_RSS,
          Accept: "image/webp,image/*,*/*;q=0.8",
          "Accept-Language": "zh-CN,zh-Hans;q=0.9",
        },
      })
    ));
  }
  private sfSecurity(): string {
    const uuid = uuidv4().toUpperCase();
    const timestamp = Math.floor(Date.now() / 1000);
    const data = `${uuid}${timestamp}${SfacgHttp.DEVICE_TOKEN}${SfacgHttp.SALT}`;
    const hash = crypto
      .createHash("md5")
      .update(data)
      .digest("hex")
      .toUpperCase();
    return `nonce=${uuid}&timestamp=${timestamp}&devicetoken=${SfacgHttp.DEVICE_TOKEN}&sign=${hash}`;
  }

  async bounusetest() {
    return await axios({
      url: `https://api.sfacg.com/user/tasks/21/advertisement?aid=43&deviceToken=${SfacgHttp.DEVICE_TOKEN}`,
      method: "put",
      data: {
        num: 1,
      },
      headers: {
        cookie:
          ".SFCommunity=D4B9C3A171728ED668F16A049EF99D2A1AEDC13C330D4ED41182A66E79D0BD0484060D95570BF74287F6EFE74DE3A22E3A73D6A9649DD3FE1DE346E0BD0EA7DB6A779CA0C45F9CAAC7E9F7770D79ED1D632F5512829A7F185D54CCE1C6ED5247; session_APP=78C2FC82A225B902DBF12548F355A2F8",
        authorization: "Basic YW5kcm9pZHVzZXI6MWEjJDUxLXl0Njk7KkFjdkBxeHE=",
        "user-agent": `boluobao/4.9.98(android;34)/H5/${SfacgHttp.DEVICE_TOKEN}/H5`,
        sfsecurity: this.sfSecurity(),
      },
    });
  }
}
