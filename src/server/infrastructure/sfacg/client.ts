import { SfacgHttpClient } from "./http";
import type {
  SfacgBookshelfCollection,
  SfacgNovel,
  SfacgSearchItem,
  SfacgUserProfile,
  SfacgVolume,
  UpstreamBookshelf,
  UpstreamSearchResponse,
  UpstreamUserInfo,
  UpstreamUserMoney,
  UpstreamVolume,
} from "./types";

/** 服务端访问 SF 小说接口的业务适配器。 */
export class SfacgApiClient extends SfacgHttpClient {
  async novelInfo(
    novelId: number,
    signal?: AbortSignal,
  ): Promise<SfacgNovel | false> {
    try {
      return await this.get<SfacgNovel>(
        `/novels/${novelId}`,
        {
          expand:
            "chapterCount,bigBgBanner,bigNovelCover,typeName,intro,fav,ticket,pointCount,sysTags,totalNeedFireMoney,latestchapter",
        },
        signal,
      );
    } catch (error) {
      this.logFailure("GET novelInfo", error);
      return false;
    }
  }

  async volumeInfos(
    novelId: number,
    signal?: AbortSignal,
  ): Promise<SfacgVolume[] | false> {
    try {
      const response = await this.get<{ volumeList: UpstreamVolume[] }>(
        `/novels/${novelId}/dirs`,
        undefined,
        signal,
      );
      return response.volumeList.map((volume) => ({
        novelId,
        volumeId: volume.volumeId,
        title: volume.title,
        chapterList: volume.chapterList.map((chapter) => ({
          ...chapter,
          volumeId: volume.volumeId,
          has:
            Boolean(chapter.has) ||
            (chapter.isVip && chapter.needFireMoney === 0),
        })),
      }));
    } catch (error) {
      this.logFailure("GET volumeInfos", error);
      return false;
    }
  }

  async contentInfos(
    chapterId: number,
    signal?: AbortSignal,
  ): Promise<string | false> {
    try {
      const response = await this.get<{ expand: { content: string } }>(
        `/Chaps/${chapterId}`,
        { expand: "content" },
        signal,
      );
      return response.expand.content;
    } catch (error) {
      this.logFailure("GET contentInfos", error);
      return false;
    }
  }

  static async image(url: string): Promise<Buffer> {
    return this.getRss<Buffer>(url);
  }

  async searchInfos(
    query: string,
    page = 0,
    size = 12,
  ): Promise<SfacgSearchItem[] | false> {
    try {
      const response = await this.get<UpstreamSearchResponse>(
        "/search/novels/result/new",
        {
          page,
          q: query,
          size,
          sort: "hot",
          searchType: 0,
        },
      );
      return [
        ...(response.novels || [])
          .filter((item) => item.novelId)
          .map((item) => this.novelItem(item)),
        ...(response.albums || [])
          .filter((item) => item.novelId)
          .map((item) => this.albumItem(item)),
        ...(response.comics || [])
          .filter((item) => item.comicId)
          .map((item) => this.comicItem(item)),
      ];
    } catch (error) {
      this.logFailure("GET searchInfos", error);
      return false;
    }
  }

  async bookshelfCollection(): Promise<SfacgBookshelfCollection | false> {
    try {
      const shelves = await this.get<UpstreamBookshelf[]>("/user/Pockets", {
        expand: "novels,albums,comics",
      });
      const categories: string[] = [];
      const items: SfacgSearchItem[] = [];
      for (const shelf of Array.isArray(shelves) ? shelves : []) {
        const bookshelfName = shelf.name || "未分类";
        categories.push(bookshelfName);
        const novels = shelf.expand?.novels || (shelf.novelId ? [shelf] : []);
        items.push(
          ...novels
            .filter((item) => item.novelId)
            .map((item) => ({
              ...this.novelItem(item),
              bookshelfName,
              bookshelfType: "novel" as const,
            })),
        );
        items.push(
          ...(shelf.expand?.albums || [])
            .filter((item) => item.novelId)
            .map((item) => ({
              ...this.albumItem(item),
              bookshelfName,
              bookshelfType: "audio" as const,
            })),
        );
        items.push(
          ...(shelf.expand?.comics || [])
            .filter((item) => item.comicId)
            .map((item) => ({
              ...this.comicItem(item),
              bookshelfName,
              bookshelfType: "comic" as const,
            })),
        );
      }
      return { categories: [...new Set(categories)], items };
    } catch (error) {
      this.logFailure("GET bookshelfCollection", error);
      return false;
    }
  }

  /** 读取当前登录账号的资料、余额与会员等级。 */
  async userProfile(): Promise<SfacgUserProfile | false> {
    try {
      const [user, money] = await Promise.all([
        this.get<UpstreamUserInfo>("/user", { expand: "welfareCoin" }),
        this.get<UpstreamUserMoney>("/user/money"),
      ]);
      if (!user.accountId) return false;
      return {
        accountId: user.accountId,
        nickName: user.nickName || "SF 用户",
        avatar: user.avatar || "",
        welfareCoin: Number(user.expand?.welfareCoin) || 0,
        fireMoneyRemain: Number(money.fireMoneyRemain) || 0,
        couponsRemain: Number(money.couponsRemain) || 0,
        vipLevel: Number(money.vipLevel) || 0,
      };
    } catch (error) {
      this.logFailure("GET userProfile", error);
      return false;
    }
  }

  private novelItem(
    item: NonNullable<UpstreamSearchResponse["novels"]>[number],
  ): SfacgSearchItem {
    return {
      authorName: item.authorName || "",
      lastUpdateTime: item.lastUpdateTime || "",
      novelCover: item.novelCover || "",
      novelId: Number(item.novelId),
      novelName: item.novelName || "未命名作品",
      typeId: item.typeId,
      bookshelfType: "novel",
    };
  }

  private albumItem(
    item: NonNullable<UpstreamSearchResponse["albums"]>[number],
  ): SfacgSearchItem {
    return {
      authorName: item.authorName || "",
      lastUpdateTime: item.lastUpdateTime || "",
      novelCover: item.coverBig || item.coverMedium || item.coverSmall || "",
      novelId: Number(item.novelId),
      novelName: item.name || "未命名有声专辑",
      bookshelfType: "audio",
    };
  }

  private comicItem(
    item: NonNullable<UpstreamSearchResponse["comics"]>[number],
  ): SfacgSearchItem {
    return {
      authorName: item.authorName || "",
      lastUpdateTime: item.lastUpdateTime || "",
      novelCover:
        item.comicCover ||
        item.coverBig ||
        item.coverMedium ||
        item.coverSmall ||
        "",
      novelId: Number(item.comicId),
      novelName: item.comicName || "未命名漫画",
      bookshelfType: "comic",
    };
  }

  private logFailure(operation: string, error: unknown) {
    const message = error instanceof Error ? error.message : "未知错误";
    console.error(`${operation} failed: ${message}`);
  }
}
