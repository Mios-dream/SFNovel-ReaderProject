
import { IaccountInfo, _dbChapters, _dbNovels } from "../types/ITypes";
import { Server } from "../../utils//db";
import { colorize } from "../../utils//tools";
import { SfacgClient } from "../api/client";
import { novelInfo } from "../types/Types";

export class _SfacgCache {

    static async UpsertNovelInfo(novel: novelInfo) {
        const { data, error } = await Server
            .from('Sfacg-novelInfos')
            .upsert({
                novelId: novel.novelId,
                novelName: novel.novelName,
                authorName: novel.authorName,
            });
        if (error) {
            console.log(`Error UpsertNovelInfo:${colorize(`${novel.novelId}`, "purple")} `, error);
            return null;
        }
        console.log(` UpsertNovelInfo successfully ${colorize(`${novel.novelId}`, "green")}`);
    }


    static async UpsertChapterInfo(chapters: _dbChapters) {
        const { data, error } = await Server
            .from('Sfacg-chapter')
            .upsert({
                chapId: chapters.chapId,
                volumeId: chapters.volumeId,
                novelId: chapters.novelId,
                ntitle: chapters.ntitle,
                content: chapters.content
            });
        if (error) {
            console.log(`Error UpsertChapterInfo: ${colorize(`${chapters.chapId}`, "purple")} `, error);
            return null;
        }
        console.log(`UpsertChapterInfo successfully ${colorize(`${chapters.chapId}`, "green")}`);
        return true
    }

    private static async UpsertAccount(accountInfo: IaccountInfo) {
        const { data, error } = await Server
            .from('Sfacg-Accounts')
            .upsert({
                userName: accountInfo.userName,
                passWord: accountInfo.passWord,
                accountId: accountInfo.accountId,
                nickName: accountInfo.nickName,
                avatar: accountInfo.avatar,
                vipLevel: accountInfo.vipLevel,
                fireMoneyRemain: accountInfo.fireMoneyRemain,
                couponsRemain: accountInfo.couponsRemain,
                cookie: accountInfo.cookie
            })
        if (error) {
            console.log(`Error UpsertAccount: ${colorize(`${accountInfo.userName}`, "purple")} `, error);
            return null;
        }
        console.log(`UpsertAccount successfully ${colorize(`${accountInfo.userName}`, "green")}`);
    }
    // 更新用户账号信息
    static async UpdateAccount(acconutInfo: IaccountInfo, newAccount: boolean = false) {
        const { userName, passWord } = acconutInfo
        const { result, anonClient } = await SfacgClient.initClient(acconutInfo, "userInfo")
        if (newAccount) {
            const Fav = await anonClient.NewAccountFavBonus()
            Fav && console.log("新号收藏任务完成")
            const Follow = await anonClient.NewAccountFollowBonus()
            Follow && console.log("新号关注任务完成")
        }
        const money = await anonClient.userMoney()
        const accountInfo = {
            userName: userName,
            passWord: passWord,
            cookie: anonClient.GetCookie(),
            ...result,
            ...money,
        };
        (result && money) ? this.UpsertAccount(accountInfo as IaccountInfo) : console.log("账号信息获取失败，请检查账号密码")
    }

    static async RemoveAccount(userName: string) {
        const { data, error } = await Server
            .from('Sfacg-Accounts')
            .delete()
            .eq('userName', userName);

        if (error) {
            console.log(`Error removeAccount: ${colorize(`${userName}`, "purple")} `, error);
            return null;
        }
        console.log(`removeAccount successfully ${colorize(`${userName}`, "green")}`);
    }


    // 返回一个包含账号密码cookie对象的列表
    static async GetallCookies() {
        const { data, error } = await Server
            .from('Sfacg-Accounts')
            .select('userName, passWord, cookie,accountId')

        if (error) {
            console.error('Error fetching cookie:', error)
            return null
        }
        return data
    }

    static async GetAccountMoney() {
        const { data, error } = await Server
            .from('Sfacg-Accounts')
            .select('userName, passWord, cookie,couponsRemain')
            .order('couponsRemain')
        if (error) {
            console.error('Error fetching cookie:', error)
            return null
        }
        return data
    }

    static async GetAccountList() {
        const { data, error } = await Server
            .from('Sfacg-Accounts')
            .select('*');

        if (error) {
            console.error('Error getAccountList:', error)
            return null
        }
        return data as IaccountInfo[]
    }



    static async GetNovelList() {
        const { data, error } = await Server
            .from('Sfacg-novelInfos')
            .select('*')
            .order("novelId")

        if (error) {
            console.error('Error fetching novelList:', error)
            return null
        }
        return data as _dbNovels[]
    }
    static async GetChapterIdsByField(fieldName: "novelId" | "volumeId", fieldValue: number) {
        let startIndex = 0;
        const pageSize = 1000; // 每次查询的行数
        let Ids: number[] = [];

        while (true) {
            const { data, error } = await Server
                .from('Sfacg-chapter')
                .select('chapId')
                .eq(fieldName, fieldValue)
                .order('chapId')
                .range(startIndex, startIndex + pageSize - 1);

            if (error) {
                console.error(`Error GetChapterIdsByField: ${fieldName}=${fieldValue}`, error);
                return null;
            }

            if (!data || data.length === 0) {
                // 如果没有更多数据，退出循环
                break;
            }

            // 添加当前批次的 IDs 至结果数组
            Ids = Ids.concat(data.map(item => item.chapId));

            // 如果返回的数据少于 pageSize，说明已经到了最后一页
            if (data.length < pageSize) {
                break;
            }

            // 更新 startIndex 以获取下一个数据批次
            startIndex += pageSize;
        }

        return Ids;
    }


    static async GetChapterContent(chapId: number) {
        const { data, error } = await Server
            .from('Sfacg-chapter')
            .select('content')
            .eq('chapId', chapId)
            .single()
        if (error) {
            console.log(`Error GetChapterContent: ${colorize(`${chapId}`, "purple")} `, error);
            return null
        }
        return data.content
    }
}



// (async () => {
//     await _SfacgCache.DownLoad(567122)

// })()


