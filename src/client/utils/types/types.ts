/** 短信服务登录接口响应。 */
export interface smsLogin { msg: string; code: number; token: string };

/** 短信服务取号接口响应。 */
export interface smsGetPhone {
    code: string;
    msg: string;
    sid: string;
    shop_name: string;
    country_name: string;
    country_code: string;
    country_qu: string;
    phone: string;
    sp: string;
    phone_gsd: string;
};





