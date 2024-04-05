import { SfacgClient } from "../api/client";
import {
  IaccountInfo,
  IexpiredInfo,
  _dbChapters,
  _selectChapters,
} from "../types/ITypes";
import { _SfacgCache } from "./cache";

// 账号排序和购买规则
export class Multi {


  // 返回按照过期日期进行排序的代币数目和ck
  async SortedInfos() {
    const _users = await _SfacgCache.GetallCookies();
    const userExpiredInfos = await Promise.all(
      (_users ?? []).map(async (user) => {
        const { result, anonClient } = await SfacgClient.initClient(
          user as IaccountInfo,
          "expireInfo"
        );
        const expiredInfos = result as IexpiredInfo[];
        let validInfos = expiredInfos.filter(
          (info) => !info.isExpired && info.has > 0
        );
        return validInfos.map((info) => {
          return {
            cookie: anonClient.cookie, // 可用ck
            has: info.has, // 快要过期的代币数量
            expireDate: info.expireDate, // 过期日期
          };
        });
      })
    );
    let allInfos = Array.prototype.concat(...userExpiredInfos);
    let sortedInfos = allInfos.sort(
      (a, b) =>
        new Date(a.expireDate).getTime() - new Date(b.expireDate).getTime()
    );
    return sortedInfos as IexpiredInfo[];
  }

  async NeedBuy(novelId: number, toOrderd: _selectChapters[]) {
    const have = await _SfacgCache.GetChapterIdsByNovelId(novelId);
    return toOrderd
      .filter((a) => !have?.includes(a.chapId))
      .sort((a, b) => a.chapId - b.chapId);
  }
  async Buy(novelId: number, toOrderd: _selectChapters[]) {
    const _buy = await this.NeedBuy(novelId, toOrderd)
    if (_buy.length === 0) {
      console.log("无未购买的章节啦！");
      return
    }
    const _sortedInfos = await this.SortedInfos();
    // 排除ck列表
    let exclude: string[] = [];
    for (const info of _sortedInfos) {
      if (_buy.length === 0) break
      // 如果没有更多章节可购买，则退出循环
      if (info.cookie && !exclude.includes(info.cookie)) {
        const client = new SfacgClient();
        client.cookie = info.cookie;
        try {
          const status = await client.orderChap(novelId, [_buy[0].chapId]);
          if (status) {
            console.log(
              "章节" + _buy[0].chapId + "：" + _buy[0].ntitle + "购买成功"
            );
            const content = await client.contentInfos(_buy[0].chapId);
            if (content) {
              await _SfacgCache.UpsertChapterInfo({ ..._buy[0], content });
              _buy.shift(); // 删除数组第一个元素
            }
          } else {
            exclude.push(info.cookie); // 不可用则排除
          }
        } catch (error) {
          console.error("购买过程中出错", error);
          // 根据错误类型决定是否将cookie加入排除列表
        }
      }
    }
    const failed = _buy.map(chap => (chap.chapId))
    console.log("还未成功购买的ID:\n" + failed.join("\n"));
    console.log("共" + failed.length + "章");

  }

  // 方便手动
  async getselectChapters(
    novelId: number,
    chapIds?: number[]
  ): Promise<_selectChapters[]> {
    const client = new SfacgClient();
    const volumes = await client.volumeInfos(novelId);
    const chaptersInfo: _selectChapters[] = [];
    if (volumes)
      // 遍历每个卷的信息
      for (const volume of volumes) {
        // 在卷中遍历每个章节
        for (const chapter of volume.chapterList) {
          // 如果章节ID在我们的chapIds数组中，收集信息
          if (!chapIds || chapIds.includes(chapter.chapId)) {
            chaptersInfo.push({
              volumeId: volume.volumeId,
              chapId: chapter.chapId,
              ntitle: chapter.ntitle,
              novelId: volume.novelId,
            });
          }
        }
      }
    return chaptersInfo;
  }
  async MultiBuy(
    novelId: number,
    chapIds?: number[]) {
    const chaptersInfo = await this.getselectChapters(novelId, chapIds)
    await this.Buy(novelId, chaptersInfo)
  }

}

(async () => {
  const mu = new Multi();
  await mu.MultiBuy(567122);

})();
