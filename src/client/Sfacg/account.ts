
import { IaccountInfo } from "./types/ITypes";
import { Server } from "../../utils/db";


export class SfacgAccountManager {

    constructor() {
        // 待改成数据库

    }
    async addAccount(accountInfo: IaccountInfo) {
        const { data, error } = await Server
            .from('Sfacg-Accounts')
            .insert([
                {
                    userName: accountInfo.userName,
                    passWord: accountInfo.passWord,
                    accountId: accountInfo.accountId,
                    nickName: accountInfo.nickName,
                    avatar: accountInfo.avatar,
                    vipLevel: accountInfo.vipLevel,
                    fireMoneyRemain: accountInfo.fireMoneyRemain,
                    couponsRemain: accountInfo.couponsRemain,
                    lastCheckIn: accountInfo.lastCheckIn,
                    cookie: accountInfo.cookie
                },
            ])
        if (error) {
            console.log('Error addAccount: ', error);
            return null;
        }
        console.log('addAccount successfully: ', data);
    }
    
    async removeAccount(userName: string) {
        const { data, error } = await Server
            .from('Sfacg-Accounts')
            .delete()
            .eq('userName', userName);

        if (error) {
            console.error('Error removeAccount: ', error);
            return null;
        }

        console.log('removeAccount successfully: ', data);
    }

    async addCheckInfo(userName: string) {
        const now = new Date();
        const isoTime = now.toISOString();

        // 使用Supabase更新Sfacg-Accounts表中的lastCheckIn字段
        const { data, error } = await Server
            .from("Sfacg-Accounts")
            .update({
                lastCheckIn: isoTime
            })
            .match({
                userName: userName
            });

        // 检查是否有错误

        if (error) {
            console.error('Error addCheckInfo: ', error);
            return null;
        }
        console.log('addCheckInfo successfully: ', data);
    }

    async cookieGet(userName: string) {
        const { data, error } = await Server
            .from('Sfacg-Accounts')
            .select('cookie')
            .eq('userName', userName)

        if (error) {
            console.error('Error fetching cookie:', error)
            return null
        }

        return data ? data[0].cookie : null
    }

    async cookieUpdate(userName: string, newCookie: string) {
        const { data, error } = await Server
            .from('Sfacg-Accounts')
            .update({ cookie: newCookie })
            .eq('userName', userName)

        if (error) {
            console.error('Error updating cookie:', error)
            return false
        }

        console.log('cookieUpdate successfully: ', data);
    }

    getAccountList() {

    }
}