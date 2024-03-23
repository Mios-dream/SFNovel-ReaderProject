
import { IaccountInfo } from "./types/ITypes";
import { Server } from "../../utils/db";

export class SfacgAccountManager {


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
        console.log('addAccount successfully');
    }

    async updateAccount(accountInfo: IaccountInfo) {
        const { data, error } = await Server
            .from('Sfacg-Accounts')
            .update({
                passWord: accountInfo.passWord,
                nickName: accountInfo.nickName,
                avatar: accountInfo.avatar,
                vipLevel: accountInfo.vipLevel,
                fireMoneyRemain: accountInfo.fireMoneyRemain,
                couponsRemain: accountInfo.couponsRemain,
                lastCheckIn: accountInfo.lastCheckIn,
                cookie: accountInfo.cookie,
            })
            .eq('userName', accountInfo.userName);
        if (error) {
            console.log('Error updateAccount: ', error);
            return null;
        }
        console.log('updateAccount successfully');
        console.log(accountInfo);

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

        console.log('removeAccount successfully', data);
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
            .eq('userName', userName);

        // 检查是否有错误

        if (error) {
            console.error('Error addCheckInfo: ', error);
            return null;
        }
        console.log('addCheckInfo successfully');
    }

    // 返回一个包含账号密码cookie对象的列表
    async allCookiesGet() {
        const { data, error } = await Server
            .from('Sfacg-Accounts')
            .select('userName, passWord, cookie,accountId')

        if (error) {
            console.error('Error fetching cookie:', error)
            return null
        }
        return data ? data : null
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

        console.log('cookieUpdate successfully');
    }

    async getAccountList() {
        const { data, error } = await Server
            .from('Sfacg-Accounts')
            .select('*');

        if (error) {
            console.error('Error getAccountList:', error)
            return false
        }
        return data
    }
}
