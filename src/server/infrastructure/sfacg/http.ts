import axios, { type AxiosRequestConfig, type AxiosResponse } from "axios";
import crypto from "node:crypto";
import { v4 as uuidv4 } from "uuid";

/** SF 上游 HTTP 协议适配器，封装认证、会话和请求签名。 */
export class SfacgHttpClient {
  static readonly host = "https://api.sfacg.com";
  static readonly webUserAgent =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0";
  static readonly rssUserAgent =
    "SFReader/4.9.76 (iPhone; iOS 16.6; Scale/3.00)";
  static readonly deviceToken = uuidv4().toUpperCase();
  private static readonly apiUserName = "androiduser";
  private static readonly apiPassword = "1a#$51-yt69;*Acv@qxq";
  private static readonly salt = "FN_Q29XHVmfV3mYX";
  private cookie: string | undefined;

  setCookie(cookie: string | undefined) {
    this.cookie = cookie;
  }

  getCookie() {
    return this.cookie;
  }

  protected async get<T>(
    url: string,
    params?: object,
    signal?: AbortSignal,
  ): Promise<T> {
    const response: AxiosResponse<{ data: T }> = await axios.get(
      url,
      this.requestConfig(params, signal),
    );
    return response.data.data;
  }

  protected async post<T>(url: string, data: unknown): Promise<T> {
    const response = await axios.post<T>(url, data, this.requestConfig());
    return url.startsWith("/session") ? (response as T) : (response.data as T);
  }

  protected async put<T>(url: string, data: unknown): Promise<T> {
    const response = await axios.put<T>(url, data, this.requestConfig());
    return response.data;
  }

  protected static async getRss<T>(url: string): Promise<T> {
    const response = await axios.get<T>(url, {
      responseType: "arraybuffer",
      baseURL: this.host,
      headers: {
        "User-Agent": this.rssUserAgent,
        Accept: "image/webp,image/*,*/*;q=0.8",
        "Accept-Language": "zh-CN,zh-Hans;q=0.9",
      },
    });
    return response.data;
  }

  private requestConfig(
    params?: object,
    signal?: AbortSignal,
  ): AxiosRequestConfig {
    return {
      withCredentials: true,
      baseURL: SfacgHttpClient.host,
      auth: {
        username: SfacgHttpClient.apiUserName,
        password: SfacgHttpClient.apiPassword,
      },
      headers: {
        cookie: this.cookie,
        Accept: "application/json, text/javascript, */*; q=0.01",
        "Accept-Language": "zh-Hans-CN;q=1",
        "User-Agent": SfacgHttpClient.webUserAgent,
        "X-Requested-With": "XMLHttpRequest",
        Referer: "https://i.sfacg.com/consume/book/",
        SFSecurity: this.securityHeader(),
      },
      signal,
      params,
    };
  }

  private securityHeader() {
    const nonce = uuidv4().toUpperCase();
    const timestamp = Math.floor(Date.now() / 1000);
    const value = `${nonce}${timestamp}${SfacgHttpClient.deviceToken}${SfacgHttpClient.salt}`;
    const sign = crypto
      .createHash("md5")
      .update(value)
      .digest("hex")
      .toUpperCase();
    return `nonce=${nonce}&timestamp=${timestamp}&devicetoken=${SfacgHttpClient.deviceToken}&sign=${sign}`;
  }
}
