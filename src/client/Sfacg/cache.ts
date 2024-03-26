
import { IaccountInfo, Ichapter, InovelInfo, IvolumeInfos } from "./types/ITypes";
import { Server } from "../../utils/db";
import { colorize } from "../../utils/tools";

export class SfacgCache {

    async UpsertNovelInfo(novel: InovelInfo) {
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
        console.log(`UpsertAccount UpsertNovelInfo ${colorize(`${novel.novelId}`, "green")}`);
    }

    async UpsertVolumeInfo(volume: IvolumeInfos) {
        const { data, error } = await Server
            .from('Sfacg-volumeInfos')
            .upsert({
                novelId: volume.novelId,
                volumeId: volume.volumeId,
                title: volume.title,
            });
        volume.chapterList.map(async (chapter: Ichapter) => {
            await this.UpsertChapterInfo(chapter)
        })
        if (error) {
            console.log(`Error UpsertVolumeInfo:${colorize(`${volume.volumeId}`, "purple")} `, error);
            return null;
        }
        console.log(`UpsertVolumeInfo successfully ${colorize(`${volume.volumeId}`, "green")}`);
    }

    async UpdateChapterContent() {

    }

    private async UpsertChapterInfo(chapter: Ichapter) {
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
    }

    async UpsertAccount(accountInfo: IaccountInfo) {
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


    async removeAccount(userName: string) {
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


