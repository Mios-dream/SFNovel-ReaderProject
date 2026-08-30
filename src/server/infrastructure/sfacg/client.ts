import axios from "axios";
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

/** 网页端未提供可下载正文时的明确错误。 */
export class SfacgWebContentError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SfacgWebContentError";
  }
}

export class SfacgApiClient extends SfacgHttpClient {
  /** 使用新的 App 签名接口读取章节正文。 */
  async chapterContentFromApi(chapterId: number, signal?: AbortSignal): Promise<string> {
    return (await this.chapterContentWithMetadataFromApi(chapterId, signal)).content;
  }

  async chapterContentWithMetadataFromApi(
    chapterId: number,
    signal?: AbortSignal,
  ): Promise<{ content: string; novelId: number; volumeId: number }> {
    const response = await this.get<{ expand?: { content?: unknown }; content?: unknown }>(
      `/Chaps/${chapterId}`,
      { expand: "content,expand.content" },
      signal,
    );
    const content = response?.expand?.content ?? response?.content;
    if (typeof content !== "string" || !content.trim())
      throw new Error("SF App API 未返回章节正文");
    const metadata = response as typeof response & { novelId?: unknown; volumeId?: unknown };
    if (!Number.isInteger(metadata.novelId) || !Number.isInteger(metadata.volumeId))
      throw new Error("SF App API 未返回章节所属作品信息");
    return {
      content,
      novelId: Number(metadata.novelId),
      volumeId: Number(metadata.volumeId),
    };
  }

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

  /**
   * 读取 SF 公开网页中实际展示的章节正文。
   * 此方法不会处理仅 App 可读或需要付费授权但网页未展示的章节。
   */
  async chapterContentFromWeb(
    novelId: number,
    volumeId: number,
    chapterId: number,
    signal?: AbortSignal,
  ): Promise<string> {
    try {
      const response = await axios.get<string>(
        `https://book.sfacg.com/Novel/${novelId}/${volumeId}/${chapterId}/`,
        {
          headers: {
            cookie: this.getCookie(),
            Accept:
              "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            "Accept-Language": "zh-CN,zh;q=0.9",
            "User-Agent": SfacgHttpClient.webUserAgent,
            Referer: `https://book.sfacg.com/Novel/${novelId}/MainIndex/`,
          },
          responseType: "text",
          signal,
          timeout: 15_000,
        },
      );
      if (/章节内容当前不可用/.test(response.data))
        throw new SfacgWebContentError(
          "SF 网页端“章节内容当前不可用”，该章节可能仅 App 可读、受限或已下架",
        );
      const body = this.chapterBodyFromHtml(response.data);
      if (!body)
        throw new SfacgWebContentError(
          "SF 网页端响应中没有找到 #ChapterBody，可能是页面结构变更或该章节不可公开访问",
        );
      return body;
    } catch (error) {
      if (error instanceof SfacgWebContentError) throw error;
      if (axios.isAxiosError(error))
        throw new SfacgWebContentError(
          `无法读取 SF 网页端正文（HTTP ${error.response?.status || "未知"}）`,
        );
      throw new SfacgWebContentError(
        error instanceof Error ? error.message : "无法读取 SF 网页端正文",
      );
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

  private chapterBodyFromHtml(html: string) {
    const match = html.match(
      /<div\b[^>]*\bid=["']ChapterBody["'][^>]*>([\s\S]*?)<\/div>/i,
    );
    if (!match) return "";
    const body = this.decodeHtml(match[1]).replace(
      /<img\b([^>]*)>/gi,
      (_tag, attributes: string) => {
        const source = attributes.match(
          /\b(?:data-original|data-src|src)\s*=\s*(["'])(.*?)\1/i,
        )?.[2];
        return source ? `\n\n![章节插图](${source})\n\n` : "";
      },
    );
    return body
      .replace(/<br\s*\/?\s*>/gi, "\n")
      .replace(/<\/p\s*>/gi, "\n")
      .replace(/<p\b[^>]*>/gi, "")
      .replace(/<[^>]+>/g, "")
      .replace(/\r\n?/g, "\n")
      .replace(/\n{3,}/g, "\n\n")
      .trim();
  }

  private decodeHtml(value: string) {
    return value
      .replace(/&nbsp;/gi, " ")
      .replace(/&amp;/gi, "&")
      .replace(/&lt;/gi, "<")
      .replace(/&gt;/gi, ">")
      .replace(/&quot;/gi, '"')
      .replace(/&#39;|&apos;/gi, "'")
      .replace(/&#(x[0-9a-f]+|\d+);/gi, (_, entity: string) => {
        const code = entity.toLowerCase().startsWith("x")
          ? Number.parseInt(entity.slice(1), 16)
          : Number.parseInt(entity, 10);
        return Number.isFinite(code) ? String.fromCodePoint(code) : _;
      });
  }
}
