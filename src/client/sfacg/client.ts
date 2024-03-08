import { SFACG } from "./http";

export class SfacgClient extends SFACG {
  async login(username: string, password: string): Promise<Boolean> {
    try {
      let res: any = await this.post("/sessions", {
        userName: username,
        passWord: password,
      });
      return res.status == 200 ? true : false;
    } catch (err: any) {
      console.error(
        `${username} LOGIN failed : ${JSON.stringify(
          err.response.data.status.msg
        )}`
      );
      return false;
    }
  }
this.
  async userInfo(): Promise<any> {
    try {
      let res: any = await this.get("/user");
      return res.data?.data ?? false;
    } catch (err: any) {
      console.error(
        `GET userInfo failed : ${JSON.stringify(err.response.data.status.msg)}`
      );
    }
  }

  async novelInfo(id: number): Promise<any> {
    try {
      let res: any = await this.get(`/novels/${id}`, {
        expand: "intro,typeName,sysTags",
      });
      return res.data?.data ?? false;
    } catch (err: any) {
      console.error(
        `GET novelInfo failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
    }
  }

  async volumeInfos(id: number): Promise<any> {
    try {
      let res: any = await this.get(`/novels/${id}/dirs`);
      return res.data?.data ?? false;
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
      let res: any = await this.get(`/Chaps/${id}`, { expand: "content" });
      return res.data?.data ?? false;
    } catch (err: any) {
      console.error(
        `GET contentInfos failed: ${JSON.stringify(
          err.response.data.status.msg
        )}`
      );
    }
  }

    async image(url: URL): Promise<any> {
        
      return
  }

  async searchInfos<T>(text: T, page: number, size: number): Promise<number[]> {
    return [];
  }

  async bookshelfInfos(): Promise<number[]> {
    return [];
  }

  async categories(): Promise<any[]> {
    return [];
  }

  async tags(): Promise<any> {
    return [];
  }

  async novels(option: any, page: number, size: number): Promise<number[]> {
    return [];
  }
}
