import { SFACG } from "./http";
import {
  bookshelfInfos,
  categories,
  contentInfos,
  novelInfo,
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
      let res: any = await this.get<novelInfo>(`/novels/${id}`, {
        expand: "intro,typeName,sysTags",
      });
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
      let res = await this.get<contentInfos>(`/Chaps/${id}`, {
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

  async image(url: string): Promise<any> {
    const response: Buffer = await this.get_rss(url);
    return response;
  }

  async searchInfos<T>(text: T, page: number, size: number): Promise<any> {
    const res = await this.get<any>("/search/novels/result/new", {
      params: {
        page,
        q: text,
        size,
        sort: "hot",
      },
    });
    return res ?? false;
  }

  async bookshelfInfos(): Promise<any> {
    const res = await this.get<bookshelfInfos>("/user/Pockets", {
      expand: "novels,albums,comics",
    });
    return res  ?? false;
  }

  async categories(): Promise<any> {
    const res = await this.get<categories>("/noveltypes");
    return res ?? false;
  }

  async tags(): Promise<any> {
    const res = await this.get<categories>("/novels/0/sysTags");
    return res ?? false;
  }

  async novels(option: any, page: number, size: number): Promise<number[]> {
    return [];
  }
}
