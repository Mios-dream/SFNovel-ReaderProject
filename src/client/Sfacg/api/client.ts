import { SfacgHttp } from "./basehttp";
import {
  adBonus,
  adBonusNum,
  androiddeviceinfos,
  AuthorInfo,
  bookshelfInfos,
  claimTask,
  contentInfos,
  expireInfo,
  NewAccountFavBonus,
  NewAccountFollowBonus,
  newSign,
  novelInfo,
  novels,
  order,
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
  welfare,
} from "../types/Types";
import {
  Itag,
  IvolumeInfos,
  Ichapter,
  IadBonusNum,
  IbookshelfCollection,
  IbookshelfInfos,
  IsearchInfos,
  IaccountInfo,
  IexpiredInfo,
} from "../types/ITypes";
import { getNowFormatDate, Secret } from "../../utils//tools";

import fs from "fs-extra";

/** SF 业务 API 客户端，负责将原始接口响应转换为应用内部数据结构。 */
export class SfacgClient extends SfacgHttp {
  /**
   * 初始化带有效 Cookie 的客户端，必要时回退到账号密码登录。
   * @param acconutInfo 账号、密码及可复用 Cookie。
   * @param todo 登录后需要调用的业务方法名。
   * @returns 业务调用结果和已初始化的客户端实例。
   */
  static async initClient(
    acconutInfo: IaccountInfo,
    todo: "getTasks" | "userInfo" | "expireInfo",
  ) {
    // 优先复用已有 Cookie，失效时再用账号密码登录，减少不必要的登录请求。
    const anonClient = new SfacgClient();
    const { userName, passWord, cookie } = acconutInfo;
    let result: any;
    if (cookie) {
      anonClient.SetCookie(cookie);
      result = await anonClient[todo]();
      result &&
        console.log(`${Secret(acconutInfo.userName as string)}原ck可用`);
    }
    if ((!cookie || !result) && userName && passWord) {
      const a = await anonClient.login(userName, passWord);
      if (a) {
        console.log(`${Secret(acconutInfo.userName as string)}ck重置`);
        result = await anonClient[todo]();
      } else {
        console.log("重新获取ck失败");
      }
    }
    return { result, anonClient };
  }

  /**
   * 使用账号密码登录 SF 并保存响应 Cookie。
   * @param userName SF 用户名。
   * @param passWord SF 密码。
   * @returns 登录成功时返回 true，否则返回 false。
   */
  async login(userName: string, passWord: string): Promise<boolean> {
    try {
      const res = await this.post<any>("/sessions", {
        userName: userName,
        passWord: passWord,
      });
      this.SetCookie(
        res.status == 200 &&
          res.headers["set-cookie"]
            .map((cookie: any) => {
              return cookie.split(";")[0];
            })
            .join("; "),
      );
      return res.status == 200;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(`POST login failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 上报 Android 设备信息，解除新账号签到时的安全风险提示。
   * @param accountId SF 账号编号。
   * @returns 接口返回成功状态时为 true。
   */
  async androiddeviceinfos(accountId: number) {
    try {
      const res = await this.post<androiddeviceinfos>(
        "/user/androiddeviceinfos",
        {
          accountId: accountId,
          package: "com.sfacg",
          abi: "arm64-v8a",
          deviceId: SfacgHttp.DEVICE_TOKEN.toLowerCase(),
          version: "4.8.22",
          deviceToken: "7b2a42976f97d470",
        },
      );
      return res.status.httpCode == 200 || 201;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(
        `POST androiddeviceinfos failed: ${JSON.stringify(errMsg)}`,
      );
      return false;
    }
  }

  /**
   * 获取当前账号的基础资料。
   * @returns 用户昵称、头像、账号编号和福利信息；请求失败时返回 false。
   */
  async userInfo() {
    try {
      const res = await this.get<userInfo>("/user", {
        expand: "welfareCoin",
      });
      // 补充用户基础信息
      const baseinfo = {
        welfare: res.expand.welfareCoin,
        nickName: res.nickName,
        avatar: res.avatar,
        accountId: res.accountId,
      };
      return baseinfo;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(`GET userInfo failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }
  /**
   * 获取当前账号的余额和会员信息。
   * @returns 火币、代币和 VIP 等余额信息；请求失败时返回 false。
   */
  async userMoney() {
    try {
      const res = await this.get<userMoney>("/user/money");
      // 补充用户余额信息
      const money = {
        fireMoneyRemain: res.fireMoneyRemain,
        couponsRemain: res.couponsRemain,
        vipLevel: res.vipLevel,
      };
      return money;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(`GET userMoney failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 查询代币剩余数量及过期时间。
   * @param page 分页页码，从 0 开始。
   * @param size 每页数量，默认 50。
   * @returns 代币过期信息数组；请求失败时返回 false。
   */
  async expireInfo(page: number = 0, size = 50) {
    try {
      const res = await this.get<expireInfo[]>("/user/coupons", {
        page: page,
        size: size,
      });
      const expire = res.map((info) => {
        return {
          has: info.coupon - info.usedCoupon,
          expireDate: info.expireDate,
          isExpired: info.isExpired,
        };
      });
      return expire as IexpiredInfo[];
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(`GET expireInfo failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 获取指定小说的详细信息。
   * @param novelId SF 小说编号。
   * @param signal 可选的取消信号。
   * @returns 小说详情；请求失败时返回 false。
   */
  async novelInfo(novelId: number, signal?: AbortSignal) {
    try {
      const res = await this.get<novelInfo>(
        `/novels/${novelId}`,
        {
          expand:
            "chapterCount,bigBgBanner,bigNovelCover,typeName,intro,fav,ticket,pointCount,sysTags,totalNeedFireMoney,latestchapter",
        },
        signal,
      );

      return res;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(`GET novelInfo failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 获取作者资料。
   * @param authorId SF 作者编号。
   * @returns 作者信息；请求失败时返回 false。
   */
  async authorInfo(authorId: number) {
    try {
      let res = await this.get<AuthorInfo>("/authors", {
        authorId: authorId,
        expand: "youfollow,fansNum",
      });

      return res;
      // 待添加
    } catch (err: any) {
      console.error(
        `GET authorInfos failed: ${JSON.stringify(
          err.response.data.status.msg,
        )}`,
      );
      return false;
    }
  }
  /**
   * 获取作者发布的作品列表。
   * @param authorId SF 作者编号。
   * @returns 作者作品数组；请求失败时返回 false。
   */
  async authorBooks(authorId: number) {
    try {
      const res = await this.get<IsearchInfos[]>(`/authors/${authorId}/novels`);
      return res;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(`GET authorBooks failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 获取小说分卷和章节目录，并转换为下载器使用的结构。
   * @param novelId SF 小说编号。
   * @param signal 可选的取消信号。
   * @returns 分卷章节列表；请求失败时返回 false。
   */
  async volumeInfos(
    novelId: number,
    signal?: AbortSignal,
  ): Promise<IvolumeInfos[] | false> {
    try {
      const res = await this.get<volumeInfos>(
        `/novels/${novelId}/dirs`,
        undefined,
        signal,
      );
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
              chapOrder: chapter.chapOrder,
              // The catalogue returns this for chapters already owned by the
              // current session. Some responses only expose a zero price.
              has:
                Boolean((chapter as unknown as { has?: boolean }).has) ||
                (chapter.isVip && chapter.needFireMoney === 0),
            };
          }),
        };
      });
      return volumeInfos;
    } catch (err: any) {
      console.error(
        `GET volumeInfos failed: ${JSON.stringify(
          err.response.data.status.msg,
        )}`,
      );
      return false;
    }
  }

  /**
   * 获取指定章节的正文内容。
   * @param chapId SF 章节编号。
   * @param signal 可选的取消信号。
   * @returns 章节正文；请求失败时返回 false。
   */
  async contentInfos(
    chapId: number,
    signal?: AbortSignal,
  ): Promise<string | false> {
    try {
      let res = await this.get<contentInfos>(
        `/Chaps/${chapId}`,
        {
          expand: "content",
        },
        signal,
      );
      const content = res.expand.content;
      return content;
      // 待添加
    } catch (err: any) {
      console.error(
        `GET contentInfos failed: ${JSON.stringify(
          err.response.data.status.msg,
        )}`,
      );
      return false;
    }
  }

  /**
   * 下载小说封面或章节插图。
   * @param url 图片资源 URL。
   * @returns 图片二进制数据；请求失败时返回 false。
   */
  static async image(url: string): Promise<any> {
    try {
      const response: Buffer = await SfacgHttp.get_rss(url);
      return Buffer.from(response);
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(`GET image failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 搜索小说、有声专辑和漫画，并统一为作品卡片结构。
   * @param novelName 搜索关键词。
   * @param page 分页页码，从 0 开始。
   * @param size 每页数量，默认 12。
   * @returns 合并后的搜索结果；请求失败时返回 false。
   */
  async searchInfos(
    novelName: string,
    page: number = 0,
    size: number = 12,
  ): Promise<IsearchInfos[] | false> {
    try {
      const res = await this.get<searchInfos>("/search/novels/result/new", {
        page: page,
        q: novelName,
        size: size,
        sort: "hot",
        searchType: 0,
      });
      const novelResults: IsearchInfos[] = (res.novels || []).map((novel) => ({
        authorName: novel.authorName,
        lastUpdateTime: novel.lastUpdateTime, // 最后更新时间
        novelCover: novel.novelCover, // 小说封面URL
        novelId: novel.novelId, // 小说ID
        novelName: novel.novelName, // 小说名称
        bookshelfType: "novel",
      }));
      const audioResults: IsearchInfos[] = (res.albums || [])
        .filter((album) => album.novelId)
        .map((album) => ({
          authorName: album.authorName || "",
          lastUpdateTime: album.lastUpdateTime,
          novelCover:
            album.coverBig || album.coverMedium || album.coverSmall || "",
          novelId: album.novelId,
          novelName: album.name || "未命名有声专辑",
          bookshelfType: "audio",
        }));
      const comicResults: IsearchInfos[] = (res.comics || []).map((comic) => ({
        authorName: comic.authorName || "",
        lastUpdateTime: comic.lastUpdateTime,
        novelCover: comic.comicCover || "",
        novelId: comic.comicId,
        novelName: comic.comicName,
        bookshelfType: "comic",
      }));
      return [...novelResults, ...audioResults, ...comicResults];
    } catch (err: any) {
      console.error(
        `GET searchInfos failed: ${JSON.stringify(
          err.response.data.status.msg,
        )}`,
      );
      return false;
    }
  }

  /**
   * 获取当前账号的书架分组及其中的小说、专辑和漫画。
   * @returns 书架分类与作品集合；请求失败时返回 false。
   */
  async bookshelfCollection(): Promise<IbookshelfCollection | false> {
    try {
      const res = await this.get<bookshelfInfos[]>("/user/Pockets", {
        expand: "novels,albums,comics",
      });
      const categories: string[] = [];
      const bookshelfItems: IbookshelfInfos[] = [];
      for (const bookshelf of Array.isArray(res) ? res : []) {
        const directNovel = bookshelf as any;
        const bookshelfName = bookshelf?.name || "未分类";
        categories.push(bookshelfName);
        const novelEntries =
          bookshelf?.expand?.novels ||
          (directNovel?.novelId ? [directNovel] : []);
        novelEntries?.forEach((novel: any) => {
          bookshelfItems.push({
            authorName: novel.authorName, // 作者名字
            lastUpdateTime: novel.lastUpdateTime, // 最后更新时间
            novelCover: novel.novelCover, // 小说封面URL
            novelId: novel.novelId, // 小说ID
            novelName: novel.novelName, // 小说名称
            bookshelfName,
            bookshelfType: "novel",
            typeId: novel.typeId,
          });
        });
        for (const album of bookshelf?.expand?.albums || []) {
          if (!album?.novelId) continue;
          bookshelfItems.push({
            authorName: album.authorName || "",
            lastUpdateTime: album.lastUpdateTime,
            novelCover:
              album.coverBig || album.coverMedium || album.coverSmall || "",
            novelId: album.novelId,
            novelName: album.name || "未命名有声专辑",
            bookshelfName,
            bookshelfType: "audio",
          });
        }
        for (const comic of bookshelf?.expand?.comics || []) {
          if (!comic?.comicId) continue;
          bookshelfItems.push({
            authorName: comic.authorName || "",
            lastUpdateTime: comic.lastUpdateTime,
            novelCover:
              comic.comicCover ||
              comic.coverBig ||
              comic.coverMedium ||
              comic.coverSmall ||
              "",
            novelId: comic.comicId,
            novelName: comic.comicName || "未命名漫画",
            bookshelfName,
            bookshelfType: "comic",
          });
        }
      }
      return { categories: [...new Set(categories)], items: bookshelfItems };
    } catch (err: any) {
      const errMsg =
        err?.response?.data?.status?.msg ||
        err?.response?.data?.message ||
        err?.message ||
        "未知错误";
      console.error(
        `GET bookshelfCollection failed: ${JSON.stringify(errMsg)}`,
      );
      return false;
    }
  }

  /**
   * 获取兼容旧版下载器的扁平小说书架列表。
   * @returns 仅包含小说类型的书架作品；请求失败时返回 false。
   */
  async bookshelfInfos(): Promise<IbookshelfInfos[] | false> {
    const collection = await this.bookshelfCollection();
    return collection
      ? collection.items.filter(
          (item) => item.bookshelfType === "novel" || !item.bookshelfType,
        )
      : false;
  }

  /**
   * 获取 SF 官方小说分类。
   * @returns 分类信息数组；请求失败时返回 false。
   */
  async typeInfo(): Promise<typeInfo[]> {
    try {
      const res = await this.get<typeInfo[]>("/noveltypes");
      return res ?? false;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(`GET typeInfo failed: ${JSON.stringify(errMsg)}`);
      throw err;
    }
  }

  /**
   * 获取系统标签并补充应用所需的百合标签。
   * @returns 标签数组。
   */
  async tags(): Promise<Itag[]> {
    try {
      const res = await this.get<tags[]>("/novels/0/sysTags");
      const tags: Itag[] = [
        // 被删手动补
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
      const errMsg = err.response.data.status.msg;
      console.error(`GET tags failed: ${JSON.stringify(errMsg)}`);
      throw err;
    }
  }

  /**
   * 获取分类主页中的小说列表。
   * @param page 分类页码。
   * @returns 分类主页作品数据。
   */
  async novels(page: number): Promise<any> {
    const res = await this.get<novels[]>(`/novels/0/sysTags/novels`, {
      page: page,
      updatedays: "-1",
      size: "20",
      isfree: "both",
      charcountbegin: "0",
      systagids: "",
      sort: "viewtimes",
      isfinish: "both",
      charcountend: "0",
    });
    const novels = res.map((novel) => {
      return {
        authorName: novel.authorName,
        lastUpdateTime: novel.lastUpdateTime, // 最后更新时间
        novelCover: novel.novelCover, // 小说封面URL
        novelId: novel.novelId, // 小说ID
        novelName: novel.novelName, // 小说名称
      };
    });
    return (res as IsearchInfos[]) ?? false;
  }

  /**
   * 使用当前会话购买指定章节。
   * @param novelId SF 小说编号。
   * @param chapId 要购买的章节 ID 数组。
   * @returns 订单创建成功时返回 true。
   */
  async orderChap(novelId: number, chapId: number[]) {
    try {
      const res = await this.post<order>(`/novels/${novelId}/orderedchaps`, {
        orderType: "readOrder",
        orderAll: false,
        autoOrder: false,
        chapIds: chapId,
      });
      return res.status.httpCode == 201;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(`orderChap failed: ${JSON.stringify(errMsg)}`);
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

  /**
   * 查询当前账号今日可领取的广告奖励次数。
   * @returns 广告任务编号、要求次数和完成次数；失败时返回 false。
   */
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
      const errMsg = err.response.data.status.msg;
      console.error(`PUT adBonusNum failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 完成一次广告任务并领取奖励。
   * @param id 广告任务编号。
   * @returns 奖励接口成功时返回 true。
   */
  async adBonus(id: number): Promise<boolean> {
    try {
      const res = await this.put<adBonus>(
        `/user/tasks/${id}/advertisement?aid=43&deviceToken=${SfacgHttp.DEVICE_TOKEN}`,
        {
          num: "1",
        },
      );
      await this.taskBonus(id);
      return res.status.httpCode == 200;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(`PUT adBonus failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 执行每日签到。
   * @returns 签到成功或当天已签到时返回 true。
   */
  async newSign() {
    try {
      const res = await this.put<newSign>("/user/newSignInfo", {
        signDate: getNowFormatDate(),
      });
      return res.status.httpCode == 200;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      if (errMsg == "该日期已签到，请重新确认并提交") {
        return true;
      }
      console.error(`PUT newSign failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 获取账号的日常任务列表。
   * @returns 任务数组；请求失败时返回 false。
   */
  async getTasks() {
    try {
      const res = await this.get<tasks[]>("/user/tasks", {
        taskCategory: 1,
        package: "com.sfacg",
        deviceToken: SfacgHttp.DEVICE_TOKEN,
        page: 0,
        size: 20,
      });
      return res;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(`GET Tasks failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 领取指定任务。
   * @param id 任务编号。
   * @returns 领取成功或任务已领取时返回 true。
   */
  async claimTask(id: number) {
    try {
      const res = await this.post<claimTask>(`/user/tasks/${id}`, {});
      return res.status.httpCode == 201;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      if (errMsg == "不能重复领取日常任务哦~") {
        return true;
      }
      console.error(`POST claimTasK${id} failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 上报阅读时长任务。
   * @param time 阅读分钟数。
   * @returns 上报成功时返回 true。
   */
  async readTime(time: number) {
    try {
      const res = await this.put<readTime>("/user/readingtime", {
        seconds: time * 60,
        entityType: 2,
        chapterId: 477385,
        entityId: 368037,
        readingDate: getNowFormatDate(),
      });
      return res.status.httpCode == 200;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(`PUT readTime failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 上报每日分享任务。
   * @param accountID SF 账号编号。
   * @returns 上报成功时返回 true。
   */
  async share(accountID: number) {
    try {
      const res = await this.put<share>(
        `/user/tasks?taskId=4&userId=${accountID}`,
        {
          env: 0,
        },
      );
      return res.status.httpCode == 200;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(`PUT share failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 领取指定任务的奖励。
   * @param id 任务编号。
   * @returns 领取成功时返回 true。
   */
  async taskBonus(id: number) {
    try {
      const res = await this.put<taskBonus>(`/user/tasks/${id}`, {});
      return res.status.httpCode == 200;
    } catch (err: any) {
      if (id == 21) {
        return false;
      }
      const errMsg = err.response.data;
      console.error(`PUT taskBonus${id} failed: ${JSON.stringify(errMsg)}`);
      return false;
    }
  }

  /**
   * 完成新账号关注推荐作者任务。
   * @returns 关注请求成功时返回 true。
   */
  async NewAccountFollowBonus() {
    try {
      const res = await this.post<NewAccountFollowBonus>("/user/follows", {
        accountIds:
          "933648,974675,2793814,3527946,3553442,3824463,6749649,6809014,7371156,",
      });
      return res.status.httpCode == 201;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(
        `POST NewAccountFollowBonus failed: ${JSON.stringify(errMsg)}`,
      );
      return false;
    }
  }

  /**
   * 完成新账号收藏推荐作品任务。
   * @returns 收藏请求成功时返回 true。
   */
  async NewAccountFavBonus() {
    try {
      const res = await this.post<NewAccountFavBonus>("/pockets/-1/novels", {
        novelId: 591904,
        categoryId: 0,
      });
      return res.status.httpCode == 201;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(
        `POST NewAccountFavBonus failed: ${JSON.stringify(errMsg)}`,
      );
      return false;
    }
  }
  /**
   * 领取指定福利记录。
   * @param recordId 福利记录编号，默认 26。
   * @returns 领取成功时返回 true。
   */
  async welfare(recordId: number = 26) {
    try {
      const res = await this.post<welfare>(
        `/user/welfare/storeitemrecords/${recordId}`,
        {},
      );
      return res.status.httpCode == 200;
    } catch (err: any) {
      const errMsg = err.response.data.status.msg;
      console.error(
        `POST NewAccountFavBonus failed: ${JSON.stringify(errMsg)}`,
      );
      return false;
    }
  }
  // async test() {
  //   const res = await this.get("https://api.sfacg.com/albums/137/chaps?expand=needFireMoney%2CoriginNeedFireMoney", { "expand": "needFireMoney,originNeedFireMoney" })
  //   fs.outputJSONSync("1.json",res )
  // }
}

// (async () => {
//   const a = new SfacgClient()
//   await a.login("13696458853", "dddd1111")
//   await a.orderChap(567122, [6981672, 6984421])
// const b = await a.expireInfo()
// fs.writeJSONSync("./TESTDATA/expireInfo.json",b)

// const acc = await a.userInfo()
// const id = acc && acc.accountId
// console.log(id);

// if (id) {
//   const info = await a.androiddeviceinfos(id)
//   console.log(info);
// }
// const b = await a.newSign()
// console.log(b);
// })();
