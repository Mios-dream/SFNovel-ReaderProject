import axios, { AxiosInstance, AxiosProxyConfig, AxiosResponse, CreateAxiosDefaults } from "axios";
import { v4 as uuidv4 } from "uuid";
import { getUid } from "./uid";

const crypto = require('crypto');


export class SFACG {

    static APP_NAME = "sfacg";
    static HOST = "https://api.sfacg.com";
    static USER_AGENT_PREFIX = "boluobao/4.9.76(iOS;16.6)/appStore/";
    static USER_AGENT_RSS = "SFReader/4.9.76 (iPhone; iOS 16.6; Scale/3.00)";
    static USERNAME = "apiuser";
    static PASSWORD = "3s#1-yt6e*Acv@qer";
    static SALT = "FMLxgOdsfxmN!Dt4";

    deviceToken?: string;
    clientconfig: CreateAxiosDefaults
    clientRssconfig: CreateAxiosDefaults

    constructor() {
        this.deviceToken = this.uid();
        this.clientconfig = {
            baseURL: SFACG.HOST,
            auth: {
                username: SFACG.USERNAME,
                password: SFACG.PASSWORD
            },
            headers: {
                'User-Agent': SFACG.USER_AGENT_PREFIX + this.deviceToken,
                'sfsecurity': this.sfSecurity(),
                'Accept': 'application/vnd.sfacg.api+json;version=1',
                'Accept-Language': 'zh-Hans-CN;q=1'
            },
        }
        this.clientRssconfig = {
            baseURL: SFACG.HOST,
            headers: {
                'User-Agent': SFACG.USER_AGENT_RSS,
                'Accept': 'image/webp,image/*,*/*;q=0.8',
                'Accept-Language': 'zh-CN,zh-Hans;q=0.9'
            },
        }
    }

    setProxy(proxyUrl: string): void {
        this.clientconfig.proxy = {
            host: new URL(proxyUrl).hostname,
            port: parseInt(new URL(proxyUrl).port),
        }
    }

    async get<T, E = any>(url: string, query?: E): Promise<T> {
        const client = this.getClient();
        let response: AxiosResponse
        try {
            if (query) {
                response = await client.get<T>(url, query);
            } else {
                response = await client.get<T>(url)
            }
            return response.data;
        }
        catch (err) {
            console.error(`An error occurred GET: ${err}`);
            throw err;
        } 
    }

    async get_rss<E>(url: string): Promise<E> {
        const client = this.getClientRss();
        let response: AxiosResponse
        try {
            response = await client.get(url)
            return response.data
        }
        catch (err) {
            console.error(`An error occurred GETt_RSS: ${err}`);
            throw err; 
        } 
    }
    async post<T, E>(url: string, data: E): Promise<T> {
        let response: AxiosResponse
        const client = this.getClient()
        try {
            response = await client.post<T>(url, data);
            return response.data;
        }
        catch(err) {
            console.error(`An error ocured POST ${(err as Error).message}`)    
            throw err; 
     }
    }

    private sfSecurity(): string {
        const uuid = uuidv4();
        const timestamp = Math.floor(Date.now() / 1000);
        const data = `${uuid}${timestamp}${this.deviceToken}${SFACG.SALT}`;
        const hash = crypto.createHash('md5').update(data).digest('hex').toUpperCase();
        return `nonce=${uuid}&timestamp=${timestamp}&devicetoken=${this.deviceToken}&sign=${hash}`;
    }

    private uid(): string {
        return getUid()
    }

    private getClient(): AxiosInstance {
        const client = axios.create(this.clientconfig);
        return client;
    }

    private getClientRss(): AxiosInstance {
        const clientRss = axios.create(this.clientRssconfig);
        return clientRss;
    }
}


