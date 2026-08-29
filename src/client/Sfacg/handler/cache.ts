
import { IaccountInfo, _dbChapters, _dbNovels } from "../types/ITypes";
import { Server } from "../../utils//db";
import { colorize } from "../../utils//tools";
import { SfacgClient } from "../api/client";
import { novelInfo } from "../types/Types";

export class _SfacgCache {

    /**
     * 写入小说基础信息到 Supabase 缓存。
     * @param novel 小说详情。
     * @returns 写入完成后的 Promise；失败时返回 null。
     */
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


    /**
     * 写入章节元数据和正文到 Supabase 缓存。
     * @param chapters 待缓存的章节数据。
     * @returns 成功时返回 true，失败时返回 null。
     */
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

    /**
     * 保存账号资料、余额和 Cookie 到账号表。
     * @param accountInfo 待保存的账号信息。
     * @returns 写入完成后的 Promise；失败时返回 null。
     */
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
    /**
     * 刷新账号 Cookie、用户资料和余额，并写入缓存。
     * @param acconutInfo 账号凭据或已有账号信息。
     * @param newAccount 是否执行新账号奖励流程。
     * @returns 账号更新完成后的 Promise。
     */
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

    /**
     * 从 Supabase 删除指定账号。
     * @param userName 要删除的用户名。
     * @returns 删除完成后的 Promise；失败时返回 null。
     */
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


    /**
     * 查询所有账号的登录凭据和 Cookie。
     * @returns 账号凭据数组；查询失败时返回 null。
     */
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

    /**
     * 按剩余代币数量升序查询账号余额。
     * @returns 账号余额数组；查询失败时返回 null。
     */
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

    /**
     * 查询完整账号列表。
     * @returns 账号信息数组；查询失败时返回 null。
     */
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



    /**
     * 查询数据库中的小说缓存列表。
     * @returns 小说缓存数组；查询失败时返回 null。
     */
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
    /**
     * 分页查询指定小说或分卷下的全部章节 ID。
     * @param fieldName 筛选字段，只支持 novelId 或 volumeId。
     * @param fieldValue 筛选字段值。
     * @returns 章节 ID 数组；查询失败时返回 null。
     */
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


    /**
     * 从数据库读取指定章节正文。
     * @param chapId 章节编号。
     * @returns 章节正文；查询失败时返回 null。
     */
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


