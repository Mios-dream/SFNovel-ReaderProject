import { SfacgHttp } from "./basehttp";
import {
  adBonus,
  adBonusNum,
  androiddeviceinfos,
  bookshelfInfos,
  claimTask,
  contentInfos,
  NewAccountFavBonus,
  NewAccountFollowBonus,
  newSign,
  novelInfo,
  readTime,
  searchInfos,
  share,
  tags,
  taskBonus,
  tasks,
  typeInfo,
  userInfo,
  userMoney,
  volumeInfos,
} from "./types/Types";
import {
  Itag,
  InovelInfo,
  IvolumeInfos,
  Ichapter,
  IadBonusNum,
  IbookshelfInfos,
  IsearchInfos,
} from "./types/ITypes";
import { getNowFormatDate } from "../../utils/tools";
import axios from "axios";




export class SfacgClient extends SfacgHttp {

  // 登录
  async login(
    userName: string,
    passWord: string,
  ): Promise<boolean> {
    const res = await this.post<any>("/sessions", {
      userName: userName,
      passWord: passWord,
    });
    this.cookie = res.status == 200 && res.headers["set-cookie"].map((cookie: any) => {
      return cookie.split(';')[0];
    }).join('; ');
    return res.status == 200
  }


  // 签到时，新号如果不加就会提示：您的账号存在安全风险
  async androiddeviceinfos(accountId: number) {
    try {
      const res = await this.post<androiddeviceinfos>("/user/androiddeviceinfos", {
        "accountId": accountId,
        "package": "com.sfacg",
        "abi": "arm64-v8a",
        "deviceId": SfacgHttp.DEVICE_TOKEN.toLowerCase(),
        "version": "4.8.22",
        "deviceToken": "7b2a42976f97d470"
      })
      return res.status.httpCode == 201
    } catch (err: any) {
      const errMsg = err.response.data.status.msg
      console.error(
        `POST NewAccountFavBonus failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }
  }

  /**
   * 仅当自提时执行
   * @returns 用户信息
   */
  async userInfo() {
    try {
      const res = await this.get<userInfo>("/user");
      // 补充用户基础信息
      const baseinfo = {
        nickName: res.nickName,
        avatar: res.avatar,
        accountId: res.accountId
      };
      return baseinfo;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg
      console.error(
        `GET userInfo failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }
  }
  /**
   * 仅当自提时执行
   * @returns
   */
  async userMoney() {
    try {
      const res = await this.get<userMoney>("/user/money");
      // 补充用户余额信息
      const money = {
        fireMoneyRemain: res.fireMoneyRemain,
        couponsRemain: res.couponsRemain,
        vipLevel: res.vipLevel
      };
      return res;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg
      console.error(
        `GET userMoney failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }
  }

  // Infos for this Novel
  async novelInfo(novelId: number): Promise<InovelInfo | false> {
    try {
      const res = await this.get<novelInfo>(`/novels/${novelId}`, {
        expand: "intro,typeName,sysTags",
      });
      const novelInfo = {
        novelId: res.novelId,
        lastUpdateTime: res.lastUpdateTime,
        novelCover: res.novelCover,
        novelName: res.novelName,
        isFinish: res.isFinish,
        authorName: res.authorName,
        charCount: res.charCount,
        intro: res.expand.intro,
        tags: res.expand.sysTags.map((tag) => tag.tagName),
      };
      return novelInfo;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg
      console.error(
        `GET novelInfo failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }
  }

  // 目录内容
  async volumeInfos(novelId: number): Promise<IvolumeInfos[] | false> {
    try {
      const res = await this.get<volumeInfos>(`/novels/${novelId}/dirs`);
      const volumeInfos = res.volumeList.map((volume): IvolumeInfos => {
        return {
          novelId: novelId,
          volumeId: volume.volumeId,
          title: volume.title,
          chapterList: volume.chapterList.map((chapter): Ichapter => {
            return {
              volumeId: volume.volumeId,
              chapId: chapter.chapId,
              needFireMoney: chapter.needFireMoney,
              isVip: chapter.isVip,
              ntitle: chapter.ntitle,
              chapOrder: chapter.chapOrder
            };
          }),
        };
      });
      return volumeInfos
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
  async contentInfos(chapId: number): Promise<string | false> {
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
      const errMsg = err.response.data.status.msg
      console.error(
        `GET image failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }
  }

  // 搜索小说
  async searchInfos(
    novelName: string,
    page: number = 0,
    size: number = 12
  ): Promise<IsearchInfos[] | false> {
    try {
      const res = await this.get<searchInfos>("/search/novels/result/new", {
        page: page,
        q: novelName,
        size: size,
        sort: "hot",
      });
      const searchInfos = res.novels.map((novel) => {
        return {
          authorName: novel.authorName,
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
  async bookshelfInfos(): Promise<IbookshelfInfos[] | false> {
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
      const errMsg = err.response.data.status.msg
      console.error(
        `GET typeInfo failed: ${JSON.stringify(errMsg)}`
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
      const errMsg = err.response.data.status.msg
      console.error(
        `GET tags failed: ${JSON.stringify(errMsg)}`
      );
      throw err;
    }
  }

  // // 获取分类主页,一堆参数没卵用，懒得写了
  // async novels(option: any): Promise<any> {
  //   const res = await this.get(`/novels/${option}/sysTags/novels`);
  //   return res ?? false;
  // }

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
      const errMsg = err.response.data.status.msg
      console.error(
        `orderChap failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }
  }

  /**
   *   
   *  TaskTime Below !
   * 。。。(> . <)。。。
   * @param novelId 
   * @param chapId 
   * @returns 
   */




  // 广告奖励次数
  async adBonusNum(): Promise<IadBonusNum | false> {
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
        taskId: res[0].taskId,
        completeNum: res[0].completeNum,
      };
      return adBonusNum;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg
      console.error(
        `PUT adBonusNum failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }
  }

  //  广告奖励
  async adBonus(id: number): Promise<boolean> {
    try {
      const res = await this.put<adBonus>(
        `/user/tasks/${id}/advertisement?aid=43&deviceToken=${SfacgHttp.DEVICE_TOKEN}`,
        {
          num: "1"
        }
      );
      await this.taskBonus(id)
      return res.status.httpCode == 200
    }
    catch (err: any) {
      const errMsg = err.response.data.status.msg
      console.error(
        `PUT adBonus failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }
  }

  // 签到
  async newSign() {
    console.log(getNowFormatDate());

    try {
      const res = await this.put<newSign>("/user/newSignInfo", {
        signDate: getNowFormatDate(),
      });
      return res.status.httpCode == 200
    } catch (err: any) {
      const errMsg = err.response.data.status.msg
      if (errMsg == "该日期已签到，请重新确认并提交") {
        return true
      }
      console.error(
        `PUT newSign failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }
  }

  // 获取任务列表
  async getTasks() {
    try {
      const res = await this.get<tasks[]>("/user/tasks", {
        taskCategory: 1,
        package: "com.sfacg",
        deviceToken: SfacgHttp.DEVICE_TOKEN,
        page: 0,
        size: 20
      })
      return res
    } catch (err: any) {
      const errMsg = err.response.data.status.msg
      console.error(
        `GET Tasks failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }

  }

  // 得到分配的任务
  async claimTask(id: number) {
    try {
      const res = await this.post<claimTask>(`/user/tasks/${id}`, {})
      return res.status.httpCode == 201
    } catch (err: any) {
      const errMsg = err.response.data.status.msg
      if (errMsg == "不能重复领取日常任务哦~") {
        return true
      }
      console.error(
        `POST claimTasK${id} failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }

  }

  // 阅读时长
  async readTime(time: number) {
    try {
      const res = await this.put<readTime>("/user/readingtime", {
        seconds: time * 60,
        entityType: 2,
        chapterId: 477385,
        entityId: 368037,
        readingDate: getNowFormatDate()
      })
      return res.status.httpCode == 200
    } catch (err: any) {
      const errMsg = err.response.data.status.msg
      console.error(
        `PUT readTime failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }

  }

  // 分享
  async share(accountID: number) {
    try {
      const res = await this.put<share>(`/user/tasks?taskId=4&userId=${accountID}`, {
        env: 0
      })
      return res.status.httpCode == 200
    } catch (err: any) {
      const errMsg = err.response.data.status.msg
      console.error(
        `PUT share failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }

  }

  // 任务得到奖励
  async taskBonus(id: number) {
    try {
      const res = await this.put<taskBonus>(`/user/tasks/${id}`, {})
      return res.status.httpCode == 200
    } catch (err: any) {
      const errMsg = err.response.data
      if (id == 21) { return false }
      console.error(
        `PUT taskBonus${id} failed:\n ${JSON.stringify(errMsg)}`
      );
      return false;
    }
  }

  async NewAccountFollowBonus() {
    try {
      const res = await this.post<NewAccountFollowBonus>("/user/follows", {
        "accountIds": "933648,974675,2793814,3527946,3553442,3824463,6749649,6809014,7371156,"
      })
      return res.status.httpCode == 201
    } catch (err: any) {
      const errMsg = err.response.data.status.msg
      console.error(
        `POST NewAccountFollowBonus failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }
  }


  async NewAccountFavBonus() {
    try {
      const res = await this.post<NewAccountFavBonus>("/pockets/-1/novels", {
        "novelId": 591904,
        "categoryId": 0
      })
      return res.status.httpCode == 201
    } catch (err: any) {
      const errMsg = err.response.data.status.msg
      console.error(
        `POST NewAccountFavBonus failed: ${JSON.stringify(errMsg)}`
      );
      return false;
    }
  }
}

// (async () => {
//   const a = new SfacgClient()
//   await a.login("17170515189", "Opooo1830")
//   const acc = await a.userInfo()
//   const id = acc && acc.accountId
//   console.log(id);

//   if (id) {
//     const info = await a.androiddeviceinfos(id)
//     console.log(info);
//   }
//   const b = await a.newSign()
//   console.log(b);
// })()

