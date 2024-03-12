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
} from "./types/ITypes";
import { SfacgOption } from "./types/ITypes";

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
      let res = await this.post<number, IaccountInfo>("/sessions", {
        userName: username,
        passWord: password,
      });
      if (option?.saveAccount) {
        let baseinfo: IaccountInfo = await this.userInfo();
        let money: IaccountInfo = await this.userMoney();
        let acconutInfo = {
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
      let res = await this.get<userInfo>("/user");
      // 补充用户基础信息
      let baseinfo = {
        nickName: res.nickName,
        avatar: res.avatar,
      };
      return baseinfo ?? false;
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
      let res = await this.get<userMoney>("/user/money");
      // 补充用户余额信息
      let money = {
        fireMoneyRemain: res.fireMoneyRemain,
        couponsRemain: res.couponsRemain,
      };
      return money ?? false;
    } catch (err: any) {
      console.error(
        `GET userMoney failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }
  }

  async novelInfo(novelId: number): Promise<InovelInfo | boolean> {
    try {
      let res = await this.get<novelInfo>(`/novels/${novelId}`, {
        expand: "intro,typeName,sysTags",
      });
      let novelInfo = {
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
      return novelInfo ?? false;
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
      let res = await this.get<volumeInfos>(`/novels/${novelId}/dirs`);
      let volumeInfos = res.volumeList.map((volume): IvolumeInfos => {
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
      let content = res.expand.content;
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
    const response: Buffer = await this.get_rss(url);
    return Buffer.from(response);
  }

  // 搜索小说
  async searchInfos(
    novelName: string,
    page: number,
    size: number
  ): Promise<any> {
    const res = await this.get<searchInfos>("/search/novels/result/new", {
      page: page,
      q: novelName,
      size: size,
      sort: "hot",
    });
    return res;
  }

  // 书架默认信息
  async bookshelfInfos(): Promise<any> {
    const res = await this.get<bookshelfInfos>("/user/Pockets", {
      expand: "novels,albums,comics",
    });

    return res ?? false;
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
      return tags ?? false;
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
  async orderChap(novelId: string, chapId: number[]) {
    const res = await this.get(`/novels/${chapId}/orderedchaps`, {
      orderType: "readOrder",
      orderAll: false,
      novelId: novelId,
      autoOrder: false,
      chapIds: [chapId],
    });
    return res ?? false;
  }

  async adBonusNum(): Promise<IadBonusNum> {
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
    return adBonusNum ?? false;
  }

  async adBonus(adBonusNum?: IadBonusNum): Promise<any | boolean> {
    const res = await this.put<any>(
      `/user/tasks/21/advertisement?aid=43&deviceToken=${SfacgHttp.DEVICE_TOKEN}`,
      {
        num: 1,
      }
    );
    return res ?? false;
  }
}
