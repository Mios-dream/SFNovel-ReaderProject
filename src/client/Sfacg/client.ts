import { SfacgHttp } from "./http";
import {
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

import { Itag, IaccountInfo, IcontentInfos, Option } from "./types/ITypes";

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
    option?: Option
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

  async novelInfo(novelId: number): Promise<any> {
    try {
      let res = await this.get<novelInfo>(`/novels/${novelId}`, {
        expand: "intro,typeName,sysTags",
      });
      return res ?? false;
    } catch (err: any) {
      console.error(
        `GET novelInfo failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
    }
  }
  // 目录内容
  async volumeInfos(novelId: number): Promise<any> {
    try {
      let res = await this.get<volumeInfos>(`/novels/${novelId}/dirs`);
      return res ?? false;
    } catch (err: any) {
      console.error(
        `GET volumeInfos failed: ${JSON.stringify(
          err.response.data.status.msg
        )}`
      );
    }
  }
  
  // 获取小说内容
  async contentInfos(chapId: number): Promise<IcontentInfos> {
    try {
      let res = await this.get<contentInfos>(`/Chaps/${chapId}`, {
        expand: "content",
      });
      let content = {
        title: res.ntitle,
        content: res.expand.content,
      };
      return content ?? false;
    } catch (err: any) {
      console.error(
        `GET contentInfos failed: ${JSON.stringify(
          err.response.data.status.msg
        )}`
      );
      throw err;
    }
  }

  // 咕咕咕。。。
  async image(url: string): Promise<any> {
    const response: Buffer = await this.get_rss(url);
    return response;
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

  async checkIn() {

  }

  async adBonus() {
    
  }
}
