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
  static readonly deviceToken = "910D166A-736E-3231-8B21-8D12DFD75F16";
  private static readonly apiUserName = "androiduser";
  private static readonly apiPassword = "1a#$51-yt69;*Acv@qxq";
  private static readonly salt = "lPQDb9AKO7$LjkPG";
  private cookie: string | undefined;
  private nonce: string | undefined;
  private noncePromise: Promise<string> | undefined;

  setCookie(cookie: string | undefined) {
    this.cookie = cookie;
  }

  getCookie() {
    return this.cookie;
  }

  setNonce(nonce: string | undefined) {
    this.nonce = nonce;
  }

  getNonce() {
    return this.nonce;
  }

  protected async get<T>(
    url: string,
    params?: object,
    signal?: AbortSignal,
  ): Promise<T> {
    const nonce = await this.ensureNonce(signal);
    const response: AxiosResponse<{ data: T }> = await axios.get(
      url,
      this.requestConfig(params, signal, nonce),
    );
    return response.data.data;
  }

  protected async post<T>(url: string, data: unknown): Promise<T> {
    const nonce = await this.ensureNonce();
    const response = await axios.post<T>(url, data, this.requestConfig(undefined, undefined, nonce));
    return url.startsWith("/session") ? (response as T) : (response.data as T);
  }

  protected async put<T>(url: string, data: unknown): Promise<T> {
    const nonce = await this.ensureNonce();
    const response = await axios.put<T>(url, data, this.requestConfig(undefined, undefined, nonce));
    return response.data;
  }

  /** 使用 App API 账号密码登录并提取上游会话 Cookie。 */
  async loginWithPassword(username: string, password: string): Promise<string> {
    const nonce = await this.ensureNonce();
    const response = await axios.post<{
      status?: { httpCode?: number; msg?: string; errorCode?: number };
    }>(`${SfacgHttpClient.host}/sessions`, { username, password, shuMeiId: "" }, {
      ...this.requestConfig(undefined, undefined, nonce),
      validateStatus: () => true,
    });
    if (response.status !== 200 || response.data?.status?.httpCode !== 200)
      throw new Error(response.data?.status?.msg || "SF 账号密码登录失败");
    const setCookie = response.headers["set-cookie"];
    const cookie = (Array.isArray(setCookie) ? setCookie : [])
      .map((value) => value.split(";", 1)[0])
      .filter((value) => /^(\.SFCommunity|session_APP)=/.test(value))
      .join("; ");
    if (!cookie) throw new Error("SF 登录成功但未返回会话 Cookie");
    this.cookie = cookie;
    return cookie;
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
    nonce?: string,
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
        Accept: "application/vnd.sfacg.api+json;version=1",
        "Accept-Charset": "UTF-8",
        "Content-Type": "application/json; charset=UTF-8",
        "User-Agent": `boluobao/5.2.16(android;35)/OPPO/${SfacgHttpClient.deviceToken.toLowerCase()}/OPPO`,
        "Accept-Encoding": "gzip",
        SFSecurity: this.securityHeader(nonce || this.nonce || uuidv4().toUpperCase()),
      },
      signal,
      params,
    };
  }

  private securityHeader(nonce: string) {
    const timestamp = Date.now();
    const repeated = Buffer.from(nonce.repeat(4), "ascii");
    const offset = (index: number) => repeated[index] - Math.floor(repeated[index] / 0x24) * 0x24;
    const reordered = Buffer.concat([
      repeated.subarray(offset(1), offset(1) + 13),
      repeated.subarray(offset(2), offset(2) + 16),
      repeated.subarray(offset(3), offset(3) + 36),
      repeated.subarray(offset(4), offset(4) + 36),
    ]);
    const auth = Buffer.from(`${timestamp}${SfacgHttpClient.salt}${SfacgHttpClient.deviceToken}${nonce}`, "ascii");
    let mixed = "";
    for (let index = 0; index < 101; index += 1)
      mixed += String.fromCharCode((auth[index] + reordered[index]) >> 1);
    const result = `${mixed.slice(65)}${mixed.slice(0, 13)}${mixed.slice(29, 65)}${mixed.slice(13, 29)}`;
    let normalized = "";
    for (const character of result) {
      const code = character.charCodeAt(0);
      if (code < 0x30)
        normalized += 0x39 < code + 19 && code + 19 < 0x41 ? String.fromCharCode(0x39) : String.fromCharCode(code + 19);
      else if ((0x39 < code && code < 0x41) || (0x5a < code && code < 0x61))
        normalized += String.fromCharCode(code + 19);
      else normalized += character;
    }
    const sign = crypto.createHash("md5").update(normalized, "utf8").digest("hex").toUpperCase();
    return `nonce=${nonce}&timestamp=${timestamp}&devicetoken=${SfacgHttpClient.deviceToken}&sign=${sign}`;
  }

  private async ensureNonce(signal?: AbortSignal) {
    if (this.nonce) return this.nonce;
    if (!this.noncePromise) {
      this.noncePromise = (async () => {
        let lastStatus = "";
        for (let attempt = 0; attempt < 3; attempt += 1) {
          const candidate = uuidv4().toUpperCase();
          const response = await axios.get(`${SfacgHttpClient.host}/Chaps/8436696?expand=content%2Cexpand.content`, {
            ...this.requestConfig(undefined, signal, candidate),
            validateStatus: () => true,
          });
          lastStatus = `${response.status}/${response.data?.status?.httpCode ?? "?"}`;
          if (response.data?.status?.httpCode !== 417) {
            this.nonce = candidate;
            return candidate;
          }
        }
        throw new Error(`SF API 未返回可用 nonce（${lastStatus}）`);
      })().finally(() => { this.noncePromise = undefined; });
    }
    return this.noncePromise;
  }
}
