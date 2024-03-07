import axios, { AxiosError, AxiosInstance, AxiosProxyConfig, AxiosResponse, CreateAxiosDefaults } from "axios";
import { v4 as uuidv4 } from "uuid";
import { wrapper } from 'axios-cookiejar-support';
import { CookieJar } from 'tough-cookie';


const crypto = require('crypto');

export class SFACG {

    static APP_NAME = "sfacg";
    static HOST = "https://api.sfacg.com";
    static USER_AGENT_PREFIX = "boluobao/4.9.76(iOS;16.6)/appStore/";
    static USER_AGENT_RSS = "SFReader/4.9.76 (iPhone; iOS 16.6; Scale/3.00)";
    static USERNAME = "apiuser";
    static PASSWORD = "3s#1-yt6e*Acv@qer";
    static SALT = "FMLxgOdsfxmN!Dt4";
    static DEVICE_TOKEN = uuidv4().toUpperCase()

    client: AxiosInstance
    clientRss: AxiosInstance
    cookieJar: CookieJar

    constructor() {
        this.cookieJar = new CookieJar();
        // 初始化axios实例
        this.client = wrapper(axios.create({
            withCredentials: true,
            baseURL: SFACG.HOST,
            auth: {
                username: SFACG.USERNAME,
                password: SFACG.PASSWORD
            },
            headers: {
                "Accept": "application/vnd.sfacg.api+json;version=1",
                "Accept-Language": "zh-Hans-CN;q=1",
                'User-Agent': SFACG.USER_AGENT_PREFIX + SFACG.DEVICE_TOKEN,
                'SFSecurity': this.sfSecurity(),
            },
        }));
        // 初始化rss client实例
        this.clientRss = wrapper(axios.create({
            baseURL: SFACG.HOST,
            headers: {
                'User-Agent': SFACG.USER_AGENT_RSS,
                'Accept': 'image/webp,image/*,*/*;q=0.8',
                'Accept-Language': 'zh-CN,zh-Hans;q=0.9'
            }
        }))
    }

    setProxy(proxyUrl: string): void {
        this.client.defaults.proxy = {
            host: new URL(proxyUrl).hostname,
            port: parseInt(new URL(proxyUrl).port),
        }
    }

    async get<T, E = any>(url: string, query?: E): Promise<T> {
        let response: any
        try {
            response = await this.client.get<T>(url, {
                jar: this.cookieJar, params: query
            })
            return response
        }   
        catch (err: any) {
            throw err
        }
    }

    async get_rss<E>(url: string): Promise<E> {

        let response: any
        try {
            response = await this.client.get(url, {
                jar: this.cookieJar
            })
            return response
        }
        catch (err: any) {
            console.error(`An error occurred GETt_RSS: ${err.data}`);
            throw err;
        }
    }
    async post<T, E>(url: string, data: E): Promise<T> {
        let response: any
        try {
            response = await this.client.post<T>(url, data, {
                jar: this.cookieJar
            });
            return response;
        }
        catch (err) {
            console.error(`An error ocured POST ${(err as Error).message}`)
            throw err;
        }
    }

    sfSecurity(): string {
        const uuid = uuidv4().toUpperCase();
        const timestamp = Math.floor(Date.now() / 1000);
        const data = `${uuid}${timestamp}${SFACG.DEVICE_TOKEN}${SFACG.SALT}`;
        const hash = crypto.createHash('md5').update(data).digest('hex').toUpperCase();
        return `nonce=${uuid}&timestamp=${timestamp}&devicetoken=${SFACG.DEVICE_TOKEN}&sign=${hash}`;
    }

}


