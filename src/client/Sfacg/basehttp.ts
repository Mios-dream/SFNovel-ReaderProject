import axios, { AxiosInstance, AxiosResponse, InternalAxiosRequestConfig } from "axios";
import { v4 as uuidv4 } from "uuid";
import crypto from "crypto"

export class SfacgHttp {
  static readonly HOST = "https://api.sfacg.com";
  static readonly USER_AGENT_RSS =
    "SFReader/4.9.76 (iPhone; iOS 16.6; Scale/3.00)";
  static readonly USERNAME = "androiduser";
  static readonly PASSWORD = "1a#$51-yt69;*Acv@qxq";
  static readonly SALT = "FMLxgOdsfxmN!Dt4";
  static readonly DEVICE_TOKEN = uuidv4().toUpperCase();

  cookie: string | undefined


  async get<T, E = any>(url: string, query?: E): Promise<T> {
    let response: AxiosResponse;
    response = await axios.get<T>(url, this._client(query));
    return url.startsWith("/sessions")
      ? response.data.status
      : response.data.data;
  }

  async get_rss<E>(url: string): Promise<E> {
    let response: AxiosResponse;
    response = await axios.get(url);
    return response.data;
  }

  async post<T, E = any>(url: string, data: E): Promise<T> {
    let response: any;
    response = await axios.post<T>(url, data, this._client());
    return url.startsWith("/session") ? response : response.data;
  }

  async put<T, E = any>(url: string, data: E): Promise<T> {
    let response: any;
    response = await axios.put<T>(url, data, this._client());
    return response.data;
  }

  getNowFormatDate(): string {
    const date = new Date();
    const year = date.getFullYear();
    const month = (date.getMonth() + 1).toString().padStart(2, "0");
    const day = date.getDate().toString().padStart(2, "0");
    return `${year}-${month}-${day}`;
  }

  private _client(query?: any): axios.AxiosRequestConfig {
    // 初始化axios实例
    return {
      withCredentials: true,
      baseURL: SfacgHttp.HOST,
      auth: {
        username: SfacgHttp.USERNAME,
        password: SfacgHttp.PASSWORD,
      },
      headers: {
        cookie: this.cookie,
        Accept: "application/vnd.sfacg.api+json;version=1",
        "Accept-Language": "zh-Hans-CN;q=1",
        "User-Agent": `boluobao/4.9.98(android;34)/H5/${SfacgHttp.DEVICE_TOKEN}/H5`,
        SFSecurity: this.sfSecurity(),
      },
      params: query,
    }
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
}
