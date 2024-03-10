import { SfacgHttp } from "./http";
import {
  bookshelfInfos,
  contentInfos,
  novelInfo,
  searchInfos,
  tags,
  typeInfo,
  userInfo,
  volumeInfos,
} from "./types/Types";

import { Itag } from "./types/ITypes";

export class SfacgClient extends SfacgHttp {
  async login(username: string, password: string): Promise<Boolean> {
    try {
      let res = await this.post<number, any>("/sessions", {
        userName: username,
        passWord: password,
      });
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
  async userInfo(): Promise<any> {
    try {
      let res = await this.get<userInfo>("/user");
      return res ?? false;
    } catch (err: any) {
      console.error(
        `GET userInfo failed : ${JSON.stringify(err.response.data.status.msg)}`
      );
    }
  }

  async novelInfo(id: number): Promise<any> {
    try {
      let res = await this.get<novelInfo>(`/novels/${id}`);
      return res ?? false;
    } catch (err: any) {
      console.error(
        `GET novelInfo failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
    }
  }

  async volumeInfos(chapId: number): Promise<any> {
    try {
      let res = await this.get<volumeInfos>(`/novels/${chapId}/dirs`);
      return res ?? false;
    } catch (err: any) {
      console.error(
        `GET volumeInfos failed: ${JSON.stringify(
          err.response.data.status.msg
        )}`
      );
    }
  }

  async contentInfos(chapId: number): Promise<any> {
    try {
      let res = await this.get<contentInfos>(`/Chaps/${chapId}`, {
        expand: "content",
      });
      return res ?? false;
    } catch (err: any) {
      console.error(
        `GET contentInfos failed: ${JSON.stringify(
          err.response.data.status.msg
        )}`
      );
    }
  }

  // 咕咕咕。。。
  async image(url: string): Promise<any> {
    const response: Buffer = await this.get_rss(url);
    return response;
  }

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
    const res = await this.get<typeInfo[]>("/noveltypes");
    return res ?? false;
  }

  async tags(): Promise<Itag[]> {
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
  }

  async novels(option: any): Promise<any> {
    const res = await this.get(`/novels/${option}/sysTags/novels`);
    return res ?? false;
  }

  async orderChap(chapId: string) {
    const res = await this.get(`/novels/${chapId}/orderedchaps`);
    return res ?? false;
  }
}
