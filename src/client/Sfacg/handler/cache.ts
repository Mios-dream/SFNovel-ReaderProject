
import { IaccountInfo, Ichapter, InovelInfo, IvolumeInfos } from "../types/ITypes";
import { Server } from "../../../utils/db";
import { colorize } from "../../../utils/tools";
import { SfacgClient } from "../api/client";





export class _SfacgCache {




    static async GetChapterFromVolumeId(volumeId: number) {
        const { data, error } = await Server
            .from('Sfacg-chapter')
            .select('*')
            .eq('volumeId', volumeId)
            .order('chapId');
        if (error) {
            console.log(`Error GetChapterFromVolumeId:`, error);
            return null;
        }
        return data
    }

    static async GetChaptersWithContent() {

    }

    static async UpdatechapterContent(chapId: number, content: string) {
        const { data, error } = await Server
            .from('Sfacg-chapter')
            .update({ content: content })
            .eq("chapId", chapId)
        if (error) {
            console.log(`Error UpsertchapterContent:${colorize(`${chapId}`, "purple")} `, error);
            return null;
        }
        console.log(` UpsertchapterContent successfully ${colorize(`${chapId}`, "green")}`);
    }

    /**
     * 更新小说除正文外所有内容
     * @param novelId 
     */
    static async UpdateNovelInfo(novelId: number) {
        const client = new SfacgClient()
        const _novelInfo = await client.novelInfo(novelId)
        _novelInfo && await _SfacgCache.UpsertNovelInfo(_novelInfo)
        const _volunms = await client.volumeInfos(novelId);
        _volunms && _volunms.map(async (volunm) => {
            await _SfacgCache.UpsertVolumeInfo(volunm)
        })
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


 

    static async UpsertNovelInfo(novel: InovelInfo) {
        const { data, error } = await Server
            .from('Sfacg-novelInfos')
            .upsert({
                novelId: novel.novelId,
                lastUpdateTime: novel.lastUpdateTime,
                novelCover: novel.novelCover,
                novelName: novel.novelName,
                isFinish: novel.isFinish,
                authorName: novel.authorName,
                charCount: novel.charCount,
                intro: novel.intro,
                tags: novel.tags
            });
        if (error) {
            console.log(`Error UpsertNovelInfo:${colorize(`${novel.novelId}`, "purple")} `, error);
            return null;
        }
        console.log(` UpsertNovelInfo successfully ${colorize(`${novel.novelId}`, "green")}`);
    }

 

    private static async UpsertVolumeInfo(volume: IvolumeInfos) {
        const { data, error } = await Server
            .from('Sfacg-volumeInfos')
            .upsert({
                novelId: volume.novelId,
                volumeId: volume.volumeId,
                title: volume.title,
            });
        const _failed: number[] = []
        volume.chapterList.map(async (chapter: Ichapter) => {
            const status = await this.UpsertChapterInfo(chapter)
            !status && _failed.push(chapter.chapId)
        })
        if (error) {
            console.log(`Error UpsertVolumeInfo:${colorize(`${volume.volumeId}`, "purple")} `, error);
            return null;
        }
        console.log(`UpsertVolumeInfo successfully ${colorize(`${volume.volumeId}`, "green")}`);
        _failed.length && console.log(`Failed to upsert ${_failed.length} chapters: ${_failed}`);
    }


    private static async UpsertChapterInfo(chapter: Ichapter) {
        const { data, error } = await Server
            .from('Sfacg-chapter')
            .upsert({
                chapId: chapter.chapId,
                needFireMoney: chapter.needFireMoney,
                isVip: chapter.isVip,
                ntitle: chapter.ntitle,
                volumeId: chapter.volumeId,
                chapOrder: chapter.chapOrder,
            });
        if (error) {
            console.log(`Error UpsertChapterInfo: ${colorize(`${chapter.chapId}`, "purple")} `, error);
            return null;
        }
        console.log(`UpsertChapterInfo successfully ${colorize(`${chapter.chapId}`, "green")}`);
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

        if (error) {
            console.error('Error fetching novelList:', error)
            return null
        }
        return data as InovelInfo[]
    }

    static async GetVolumeList(novelId: number) {
        const { data, error } = await Server
            .from('Sfacg-volumeInfos')
            .select('*')
            .eq('novelId', novelId)

        if (error) {
            console.error('Error fetching volumeList:', error)
            return null
        }
        return data as IvolumeInfos[]
    }

    static async GetChapterNoContent(novelId: number) {
        // 首先，从Sfacg-volumeInfos表获取所有匹配novelId的volumeId
        const volumeResponse = await Server
            .from('Sfacg-volumeInfos')
            .select('volumeId')
            .eq('novelId', novelId)

        if (volumeResponse.error) {
            console.error('Error fetching volumeIds:', volumeResponse.error)
            return
        }
        // 提取volumeId数组
        const volumeIds = volumeResponse.data.map((v: { volumeId: any; }) => v.volumeId)
        console.log(volumeIds);

        // 然后，使用volumeIds查询Sfacg-chapter表中content为空的章节
        const { data, error } = await Server
            .from('Sfacg-chapter')
            .select('chapId, needFireMoney')
            .is('content', null)
            .in('volumeId', volumeIds)
            .order("chapId")

        if (error) {
            console.error('Error fetching chapters:', error)
            return
        }
        // 输出查询结果
        return data
    }

}



// (async () => {
//     await _SfacgCache.DownLoad(567122)

// })()


