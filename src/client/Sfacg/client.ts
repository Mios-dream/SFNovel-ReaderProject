import { SFACG } from "./http";
import {
  bookshelfInfos,
  categories,
  contentInfos,
  novelInfo,
  tags,
  userInfo,
  volumeInfos,
} from "./types/Types";

export class SfacgClient extends SFACG {
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

  async volumeInfos(id: number): Promise<any> {
    try {
      let res = await this.get<volumeInfos>(`/novels/${id}/dirs`);
      return res ?? false;
    } catch (err: any) {
      console.error(
        `GET volumeInfos failed: ${JSON.stringify(
          err.response.data.status.msg
        )}`
      );
    }
  }

  async contentInfos(id: number): Promise<any> {
    try {
      let res = await this.get<contentInfos>(`/Chaps/${id}`);
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

  async searchInfos(text: string, page: number, size: number): Promise<any> {
    const res = await this.get<any>("/search/novels/result/new", {
      page: page,
      q: text,
      size: size,
      sort: "hot",
    });
    return res;
  }

  async bookshelfInfos(): Promise<any> {
    const res = await this.get<bookshelfInfos>("/user/Pockets", {
      expand: "novels,albums,comics",
    });
    return res ?? false;
  }

  async categories(): Promise<any> {
    const res = await this.get<categories>("/noveltypes");
    return res ?? false;
  }

  async tags(): Promise<any> {
    const res = await this.get<tags>("/novels/0/sysTags");
    /**
     * // // tags to add // //
     * {
     *     id: 74
     *     name: "百合"
     * }
     */
    return res ?? false;
  }

  async novels(categoryid: string): Promise<any> {
    const res = await this.get(`/novels/${categoryid}/sysTags/novels`);
    return res ?? false;
  }
}
