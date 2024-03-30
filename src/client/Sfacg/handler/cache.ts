
import { IaccountInfo, Ichapter, InovelInfo, IvolumeInfos, _dbChapters, _dbNovels } from "../types/ITypes";
import { Server } from "../../../utils/db";
import { colorize } from "../../../utils/tools";
import { SfacgClient } from "../api/client";





export class _SfacgCache {

    private static async UpsertNovelInfo(novel: InovelInfo) {
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
            cookie: anonClient.cookie,
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

    static async GetChapterIdsByVolumeId(volumeId: number) {
        const { data, error } = await Server
            .from('Sfacg-chapter')
            .select('chapId')
            .eq('volumeId', volumeId)
            .order('chapId')
        if (error) {
            console.log(`Error GetChapterIdsByvolumeId: ${colorize(`${volumeId}`, "purple")} `, error);
            return null
        }

        const Ids = data.map(item => item.chapId);
        return Ids as number[]
    }
    static async GetChapterIdsByNovelId(novelId: number) {
        const { data, error } = await Server
            .from('Sfacg-chapter')
            .select('chapId')
            .eq('novelId', novelId)
            .order('chapId')
        if (error) {
            console.log(`Error GetChapterIdsBynovelId: ${colorize(`${novelId}`, "purple")} `, error);
            return null
        }
        const Ids = data.map(item => item.chapId);
        return Ids as number[]
    }


    static async GetChapterContent(chapId: number) {
        const { data, error } = await Server
            .from('Sfacg-chapter')
            .select('ntitle,content')
            .eq('chapId', chapId)
            .single()
        if (error) {
            console.log(`Error GetChapterContent: ${colorize(`${chapId}`, "purple")} `, error);
            return null
        }
        return data as _dbChapters
    }
}



// (async () => {
//     await _SfacgCache.DownLoad(567122)

// })()


