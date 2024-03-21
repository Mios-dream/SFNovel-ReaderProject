export interface smsLogin { msg: string; code: number; token: string };

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

export interface saveAccountInfo {
    data: any
}





