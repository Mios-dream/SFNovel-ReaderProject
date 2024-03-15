import { SfacgHttp } from "./http";
import {
  adBonusNum,
  bookshelfInfos,
  contentInfos,
  novelInfo,
  searchInfos,
  tags,
  typeInfo,
  userInfo,
  userMoney,
  volumeInfos,
} from "./types/Types";

import {
  Itag,
  IaccountInfo,
  InovelInfo,
  IvolumeInfos,
  Ichapter,
  IadBonusNum,
  SfacgOption,
  IbookshelfInfos,
  IsearchInfos,
} from "./types/ITypes";

export class SfacgClient extends SfacgHttp {


  /**
   *
   * @param username  用户名
   * @param password  密码
   * @returns  登录状态
   */
  async login(
    username: string,
    password: string,
    option?: SfacgOption
  ): Promise<IaccountInfo | boolean> {
    try {
      const res = await this.post<number, IaccountInfo>("/sessions", {
        userName: username,
        passWord: password,
      });
      if (option?.saveAccount) {
        const baseinfo: IaccountInfo = await this.userInfo();
        const money: IaccountInfo = await this.userMoney();
        const acconutInfo = {
          userName: username,
          passWord: password,
          ...baseinfo,
          ...money,
        };
        return acconutInfo;
      }
      return res == 200 ? true : false;
    } catch (err: any) {
      console.error(
        `${username} LOGIN failed : ${JSON.stringify(
          err.response.data.status.msg
        )}`
      );
      return false;
    }
  }
  /**
   * 仅当login的保存选项被打开时执行
   * @returns 用户信息
   */
  async userInfo(): Promise<any> {
    try {
      const res = await this.get<userInfo>("/user");
      // 补充用户基础信息
      const baseinfo = {
        nickName: res.nickName,
        avatar: res.avatar,
      };
      return baseinfo;
    } catch (err: any) {
      console.error(
        `GET userInfo failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }
  }
  /**
   * 仅当login的保存选项被打开时执行
   * @returns
   */
  async userMoney(): Promise<any> {
    try {
      const res = await this.get<userMoney>("/user/money");
      // 补充用户余额信息
      const money = {
        fireMoneyRemain: res.fireMoneyRemain,
        couponsRemain: res.couponsRemain,
      };
      return money;
    } catch (err: any) {
      console.error(
        `GET userMoney failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }
  }

  // Infos for this Novel
  async novelInfo(novelId: number): Promise<InovelInfo | boolean> {
    try {
      const res = await this.get<novelInfo>(`/novels/${novelId}`, {
        expand: "intro,typeName,sysTags",
      });
      const novelInfo = {
        lastUpdateTime: res.lastUpdateTime,
        novelCover: res.novelCover,
        bgBanner: res.bgBanner,
        novelName: res.novelName,
        isFinish: res.isFinish,
        authorName: res.authorName,
        charCount: res.charCount,
        intro: res.expand.intro,
        tags: res.expand.sysTags.map((tag) => tag.tagName),
      };
      return novelInfo;
    } catch (err: any) {
      console.error(
        `GET novelInfo failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }
  }

  // 目录内容
  async volumeInfos(novelId: number): Promise<IvolumeInfos[] | boolean> {
    try {
      const res = await this.get<volumeInfos>(`/novels/${novelId}/dirs`);
      const volumeInfos = res.volumeList.map((volume): IvolumeInfos => {
        return {
          volumeId: volume.volumeId,
          title: volume.title,
          chapterList: volume.chapterList.map((chapter): Ichapter => {
            return {
              chapId: chapter.chapId,
              needFireMoney: chapter.needFireMoney,
              charCount: chapter.charCount,
              chapOrder: chapter.chapOrder,
              isVip: chapter.isVip,
              ntitle: chapter.ntitle,
            };
          }),
        };
      });
      return volumeInfos ?? false;
    } catch (err: any) {
      console.error(
        `GET volumeInfos failed: ${JSON.stringify(
          err.response.data.status.msg
        )}`
      );
      return false;
    }
  }

  // 获取小说内容
  async contentInfos(chapId: number): Promise<string | boolean> {
    try {
      let res = await this.get<contentInfos>(`/Chaps/${chapId}`, {
        expand: "content",
      });
      const content = res.expand.content;
      return content;
      // 待添加
    } catch (err: any) {
      console.error(
        `GET contentInfos failed: ${JSON.stringify(
          err.response.data.status.msg
        )}`
      );
      return false;
    }
  }

  async image(url: string): Promise<any> {
    try {
      const response: Buffer = await this.get_rss(url);
      return Buffer.from(response);
    } catch (err: any) {
      console.error(
        `GET image failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }
  }

  // 搜索小说
  async searchInfos(
    novelName: string,
    page: number,
    size: number
  ): Promise<IsearchInfos[] | boolean> {
    try {
      const res = await this.get<searchInfos>("/search/novels/result/new", {
        page: page,
        q: novelName,
        size: size,
        sort: "hot",
      });
      const searchInfos = res.novels.map((novel) => {
        return {
          authorId: novel.authorId, // 作者ID
          lastUpdateTime: novel.lastUpdateTime, // 最后更新时间
          novelCover: novel.novelCover, // 小说封面URL
          novelId: novel.novelId, // 小说ID
          novelName: novel.novelName, // 小说名称
        };
      });
      return searchInfos;
    } catch (err: any) {
      console.error(
        `GET searchInfos failed: ${JSON.stringify(
          err.response.data.status.msg
        )}`
      );
      return false;
    }
  }

  // 书架默认信息
  async bookshelfInfos(): Promise<IbookshelfInfos[] | boolean> {
    const res = await this.get<bookshelfInfos[]>("/user/Pockets", {
      expand: "novels,albums,comics",
    });
    let bookshelfInfos: IbookshelfInfos[] = [];
    res.map((bookshelf) => {
      let novels = bookshelf.expand.novels;
      if (novels) {
        novels.forEach((novel) => {
          bookshelfInfos.push({
            authorId: novel.authorId, // 作者ID
            lastUpdateTime: novel.lastUpdateTime, // 最后更新时间
            novelCover: novel.novelCover, // 小说封面URL
            novelId: novel.novelId, // 小说ID
            novelName: novel.novelName, // 小说名称
          });
        });
      }
    });
    return bookshelfInfos;
  }

  // 筛选分类信息
  async typeInfo(): Promise<typeInfo[]> {
    try {
      const res = await this.get<typeInfo[]>("/noveltypes");
      return res ?? false;
    } catch (err: any) {
      console.error(
        `GET typeInfo failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      throw err;
    }
  }

  // 获取所有标签
  async tags(): Promise<Itag[]> {
    try {
      const res = await this.get<tags[]>("/novels/0/sysTags");
      const tags: Itag[] = [
        {
          id: 74,
          name: "百合",
        },
      ];
      res.map((tag) => {
        tags.push({
          id: tag.sysTagId,
          name: tag.tagName,
        });
      });
      return tags;
    } catch (err: any) {
      console.error(
        `GET tags failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      throw err;
    }
  }

  // 获取分类主页
  async novels(option: any): Promise<any> {
    const res = await this.get(`/novels/${option}/sysTags/novels`);
    return res ?? false;
  }

  // 购买章节
  async orderChap(novelId: string, chapId: number[]): Promise<any> {
    try {
      const res = await this.get(`/novels/${chapId}/orderedchaps`, {
        orderType: "readOrder",
        orderAll: false,
        novelId: novelId,
        autoOrder: false,
        chapIds: [chapId],
      });
      return res;
    } catch (err: any) {
      console.error(
        `orderChap failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }
  }
  // 广告奖励次数
  async adBonusNum(): Promise<IadBonusNum | boolean> {
    try {
      const res = await this.get<adBonusNum[]>(`user/tasks`, {
        taskCategory: 5,
        package: "com.sfacg",
        deviceToken: SfacgHttp.DEVICE_TOKEN,
        page: 0,
        size: 20,
      });
      const adBonusNum = {
        requireNum: res[0].requireNum,
        completeNum: res[0].completeNum,
        taskId: res[0].taskId,
      };
      return adBonusNum;
    } catch (err: any) {
      console.error(
        `PUT adBonusNum failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }
  }
  //  广告奖励
  async adBonus(): Promise<any | boolean> {

      const res = await this.put<any>(
        `/user/tasks/21/advertisement?aid=43&deviceToken=${SfacgHttp.DEVICE_TOKEN}`,
        {
          num:"1"
        }
      );
      return res;

  }
  // 签到
  async newSign() {
    try {
      const res = await this.put("/user/newSignInfo", {
        signDate: this.getNowFormatDate(),
      });
      return res;
    } catch (err: any) {
      console.error(
        `PUT newSign failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }
  }

}
