import { SfacgHttp } from "./basehttp";
import {
  adBonus,
  adBonusNum,
  bookshelfInfos,
  claimTask,
  codeverify,
  contentInfos,
  nameAvalible,
  newSign,
  novelInfo,
  readTime,
  regist,
  searchInfos,
  sendCode,
  tags,
  tasks,
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
  IbookshelfInfos,
  IsearchInfos,
} from "./types/ITypes";
import { sms } from "../../utils/sms";
import { SfacgAccountManager } from "./account";



export class SfacgClient extends SfacgHttp {
  accountManager: SfacgAccountManager

  constructor() {
    super();
    this.accountManager = new SfacgAccountManager()
  }


  // async init(userName: string, passWord: string): Promise<void> {
  //   if (Config.Sfacg.saveAccount) {
  //     let cookie = this.accountManager.cookieGet(userName)
  //     if (cookie) {
  //       this.cookieJar.setCookieSync(cookie, SfacgHttp.HOST)
  //       console.log("正在校验ck有效性")
  //       if (!await this.userInfo()) {
  //         console.log("ck无效，正在尝试重新登录")
  //         await this.login(userName, passWord)
  //         this.accountManager.cookieSave(userName, this.cookieJar.getCookieStringSync(SfacgHttp.HOST))
  //       }
  //       else {
  //         console.log("有效cookie")
  //       }
  //     }
  //     else {
  //       const acconutInfo = await this.login(userName, passWord) as IaccountInfo
  //       this.accountManager.addAccount(acconutInfo)
  //     }
  //   }
  // }
  // 登录
  async login(
    userName: string,
    passWord: string,
  ): Promise<IaccountInfo | boolean> {
    const res = await this.post<number, IaccountInfo>("/sessions", {
      userName: userName,
      passWord: passWord,
    });
    return res == 200
  }


  /**
   * 仅当login的保存选项被打开时执行
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
  async userMoney() {
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
      console.error(
        `GET novelInfo failed: ${JSON.stringify(err.response.data.status.msg)}`
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
          volumeId: volume.volumeId,
          title: volume.title,
          chapterList: volume.chapterList.map((chapter): Ichapter => {
            return {
              chapId: chapter.chapId,
              needFireMoney: chapter.needFireMoney,
              isVip: chapter.isVip,
              ntitle: chapter.ntitle,
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
      console.error(
        `GET image failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }
  }

  // 搜索小说
  async searchInfos(
    novelName: string,
    page: number = 1,
    size: number = 1
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
      console.error(
        `orderChap failed: ${JSON.stringify(err.response.data.status.msg)}`
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


  // async Tasker() {
  //   // adBonusNum, newSign, getTasks, claimTask

  //   // adBonus, readTime, share

  //   // taskBonus
  //   await this.newSign()
  //   const tasks = await this.getTasks() as number[]
  //   tasks.map(async (taskId) => {
  //     await this.claimTask(taskId)
  //   })
  //   await this.readTime(120)
  //   await this.share(userId)
  //   tasks.map(async (taskId) => {
  //     await this.taskBonus(taskId)
  //   })
  //   const { taskId, requireNum } = await this.adBonusNum() as IadBonusNum
  //   await this.claimTask(taskId)
  //   console.log(`需要观看广告的次数：${requireNum} `)
  //   for (let i = 0; i < requireNum; i++) {
  //     await this.adBonus(taskId)
  //     await this.taskBonus(taskId)
  //   }
  // }


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
      console.error(
        `PUT adBonus failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }
  }

  // 签到
  async newSign() {
    try {
      const res = await this.put<newSign>("/user/newSignInfo", {
        signDate: this.getNowFormatDate(),
      });
      return res.status.httpCode == 200
    } catch (err: any) {
      console.error(
        `PUT newSign failed: ${JSON.stringify(err.response.data.status.msg)}`
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
      const tasks = res.filter((task) => task.status == 0)
      return tasks.map(task => (
        task.taskId
      ))
    } catch (err: any) {
      console.error(
        `GET Tasks failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }

  }

  // 得到分配的任务
  async claimTask(id: number) {
    try {
      const res = await this.post<claimTask>(`/user/tasks/${id}`, {})
      return res
    } catch (err: any) {
      console.error(
        `POST claimTasK${id} failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }

  }

  // 阅读时长
  async readTime(time: number) {
    try {
      const res = await this.put<readTime>("/user/readingtime", {
        seconds: time,
        entityType: 2,
        chapterId: 477385,
        entityId: 368037,
        readingDate: this.getNowFormatDate()
      })
      return res.status.httpCode == 200
    } catch (err: any) {
      console.error(
        `PUT readTime failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }

  }

  // 分享
  async share(userId: number) {
    try {
      const res = this.put(`/user/tasks?taskId=4&userId=${userId}`, {
        env: 0
      })
      return res
    } catch (err: any) {
      console.error(
        `PUT share failed: ${JSON.stringify(err.response.data.status.msg)}`
      );
      return false;
    }

  }

  // 任务得到奖励
  async taskBonus(id: number) {
    const res = await this.put<any>(`/user/tasks/${id}`, {})
  }

}



export class SfacgRegister extends SfacgHttp {
  phone: number = 0;
  sms: sms;
  constructor() {
    super();
    this.sms = new sms();
  }

  async avalibleNmae(name: string): Promise<boolean> {
    const res = await this.post<nameAvalible>("/users/availablename", {
      nickName: name,
    });
    return res.data.nickName.valid;
  }
  async sendCode() {
    this.phone = await this.sms.sms(50896);
    const res = await this.post<sendCode>(`/sms/${this.phone}/86`, "");
    return res.status.httpCode == 201;
  }

  async codeverify(phoneNum: number, smsAuthCode: number) {
    const res = await this.put<codeverify>(`/sms/${phoneNum}/86`, {
      smsAuthCode: smsAuthCode,
    });
    return res.status.httpCode == 200
  }

  async regist(
    passWord: string,
    nickName: string,
    phoneNum: number,
    smsAuthCode: number
  ) {
    let res = await this.post<regist>("/user", {
      passWord: passWord,
      nickName: nickName,
      countryCode: "86",
      phoneNum: phoneNum,
      email: "",
      smsAuthCode: smsAuthCode,
      shuMeiId: "",
    });
    let accountID = res.data.accountId;
    return accountID;
  }
}



