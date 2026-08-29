import axios, { AxiosResponse } from "axios";
import { v4 as uuidv4 } from "uuid";
import crypto from "crypto";

/** SF API 的底层 HTTP 封装：统一认证头、Cookie、签名和响应解包规则。 */
export class SfacgHttp {
  static readonly HOST = "https://api.sfacg.com";
  static readonly USER_AGENT_WEB =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0";
  static readonly USER_AGENT_RSS =
    "SFReader/4.9.76 (iPhone; iOS 16.6; Scale/3.00)";
  static readonly USERNAME = "androiduser";
  static readonly PASSWORD = "1a#$51-yt69;*Acv@qxq";
  static readonly SALT = "FN_Q29XHVmfV3mYX"; // new Salt for Sfacg 5.0 Upper
  static readonly DEVICE_TOKEN = uuidv4().toUpperCase();
  private cookie: string | undefined;

  /**
   * 设置当前客户端后续请求使用的 SF Cookie。
   * @param cookie SF 会话 Cookie 字符串。
   * @returns 无返回值。
   */
  SetCookie(cookie: string) {
    this.cookie = cookie;
  }

  /**
   * 获取当前客户端保存的 SF Cookie。
   * @returns Cookie 字符串；尚未登录时返回 undefined。
   */
  GetCookie() {
    return this.cookie;
  }
  /**
   * 发起经过统一认证和签名处理的 GET 请求。
   * @param url API 相对路径。
   * @param query 可选查询参数。
   * @param signal 可选取消信号。
   * @returns 解包后的接口数据。
   */
  protected async get<T, E = any>(
    url: string,
    query?: E,
    signal?: AbortSignal,
  ): Promise<T> {
    let response: AxiosResponse;
    response = await axios.get<T>(url, this._client(query, signal));
    return url.startsWith("/sessions")
      ? response.data.status
      : response.data.data;
  }

  /**
   * 请求图片等 RSS/静态资源接口。
   * @param url 资源 URL。
   * @returns 原始响应数据。
   */
  protected static async get_rss<E>(url: string): Promise<E> {
    let response: AxiosResponse;
    response = await axios.get(url, this._clientRss());
    return response.data;
  }

  /**
   * 发起 JSON POST 请求。
   * @param url API 相对路径。
   * @param data 请求体数据。
   * @returns 解包后的接口响应。
   */
  protected async post<T, E = any>(url: string, data: E): Promise<T> {
    let response: any;
    response = await axios.post<T>(url, data, this._client());
    return url.startsWith("/session") ? response : response.data;
  }

  /**
   * 发起 JSON PUT 请求。
   * @param url API 相对路径。
   * @param data 请求体数据。
   * @returns 解包后的接口响应。
   */
  protected async put<T, E = any>(url: string, data: E): Promise<T> {
    let response: any;
    response = await axios.put<T>(url, data, this._client());
    return response.data;
  }

  /**
   * 发起 HEAD 请求。
   * @param url API 相对路径。
   * @returns 响应数据。
   */
  protected async head<T>(url: string): Promise<T> {
    let response: any;
    response = await axios.head<T>(url, this._client());
    return response.data;
  }

  /**
   * 构造 API 请求的 Axios 配置。
   * @param query 可选查询参数。
   * @param signal 可选取消信号。
   * @returns Axios 请求配置对象。
   */
  private _client(query?: any, signal?: AbortSignal): any {
    // API 的账号密码是官方客户端协议要求的基础认证，用户 Cookie 负责具体会话。
    return {
      withCredentials: true,
      baseURL: SfacgHttp.HOST,
      auth: {
        username: SfacgHttp.USERNAME,
        password: SfacgHttp.PASSWORD,
      },
      headers: {
        cookie: this.cookie,
        Accept: "application/json, text/javascript, */*; q=0.01",
        "Accept-Language": "zh-Hans-CN;q=1",
        "User-Agent": SfacgHttp.USER_AGENT_WEB,
        "X-Requested-With": "XMLHttpRequest",
        Referer: "https://i.sfacg.com/consume/book/",
        SFSecurity: this.sfSecurity(),
      },
      signal,
      params: query,
    };
  }

  /**
   * 构造静态资源请求的 Axios 配置。
   * @returns RSS/图片请求配置对象。
   */
  private static _clientRss(): any {
    return {
      responseType: "arraybuffer",
      baseURL: SfacgHttp.HOST,
      headers: {
        "User-Agent": SfacgHttp.USER_AGENT_RSS,
        Accept: "image/webp,image/*,*/*;q=0.8",
        "Accept-Language": "zh-CN,zh-Hans;q=0.9",
      },
    };
  }
  /**
   * 生成 SF API 所需的请求签名。
   * @returns 包含 nonce、时间戳和签名的请求头值。
   */
  private sfSecurity(): string {
    // 每次请求生成新的 nonce 和时间戳，组合设备信息后计算 SF 使用的 MD5 签名。
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
