import { SfacgClient } from "../api/client";
import {
    InovelInfo,
    IsearchInfos,
    IvolumeInfos,
} from "../types/ITypes";
import { _SfacgCache } from "./cache";
import fse from "fs-extra";
import path from "path";
import Table from "cli-table3";
import { colorize, question, questionAccount } from "../../../utils/tools";

const outputDir = path.join(process.cwd(), "output", "菠萝包轻小说");

export class _SfacgDownloader {
    /** 
     * 处理初始化，根据cookie分发下载任务（数据库/个人下载）
     * @param novelId 小说ID
     * @param cookie sf 用户凭证
     */
    static async DownLoad(novelId: number, cookie?: string) {
        let content: string = "";
        const _client = new SfacgClient();
        const _novelInfo = await _client.novelInfo(novelId);
        _client.cookie = cookie;
        const _volumeInfos = await _client.volumeInfos(novelId);

        if (_novelInfo && _volumeInfos) {
            // 初始化路径等
            const head = await this.markdownHead(_novelInfo);
            const imagesDir = path.join(outputDir, _novelInfo.novelName, "imgs");
            const novelPath = path.join(
                outputDir,
                _novelInfo.novelName,
                `${_novelInfo.novelName}.md`
            );
            await fse.ensureDir(imagesDir);
            await this.imgDownload("cover", imagesDir, _novelInfo.novelCover);

            // 使用 Promise.all 来等待所有下载的完成
            const downloadPromises = _volumeInfos.map(_volumeInfo => {
                return cookie
                    ? this.UserDownload(_volumeInfo, imagesDir, cookie)
                    : this.ServerDownload(_volumeInfo.volumeId, imagesDir)
                        .then(volumeContent => {
                            return "# " + _volumeInfo.title + "\n\n" + volumeContent;
                        });
            });
            // 等待所有章节内容下载完成
            const volumesContent = await Promise.all(downloadPromises);
            content += volumesContent.join("\n\n");

            // 写入文件
            await fse.outputFile(novelPath, head + content);
        }
    }

    private static async UserDownload(
        volumeInfo: IvolumeInfos,
        imagesDir: string,
        cookie: string
    ): Promise<string> {
        const _client = new SfacgClient();
        _client.cookie = cookie;
        let content: string = "";

        // 创建一个Promise数组来处理每一章的下载
        const chapterDownloadPromises = volumeInfo.chapterList.map(async (_chapter) => {
            // 仅下载已购买的章节
            if (_chapter.needFireMoney === 0) {
                const chapterContent = await _client.contentInfos(_chapter.chapId);
                if (chapterContent) {
                    let formattedContent = "## " + _chapter.ntitle + "\n";
                    formattedContent += await this.ParseImg(chapterContent, imagesDir, _chapter.chapId);
                    return formattedContent;
                }
            }
            return ""; // 如果不需要下载，则返回空字符串
        });

        // 等待所有章节下载完成
        const chaptersContent = await Promise.all(chapterDownloadPromises);

        // 拼接所有章节内容
        content += chaptersContent.join("\n\n");
        return content;
    }

    static async ServerDownload(volumeId: number, imagesDir: string) {
        const _ids = await _SfacgCache.GetChapterIdsByVolumeId(volumeId);
        let content: string = "";
        if (_ids) {
            // 使用map和Promise.all来处理每一章的下载
            const chapterDownloadPromises = _ids.map(async (_id) => {
                const _chapter = await _SfacgCache.GetChapterContent(_id);
                if (_chapter) {
                    console.log("正在下载" + _chapter.ntitle);
                    let chapterContent = "## " + _chapter.ntitle + "\n";
                    chapterContent += await this.ParseImg(_chapter.content, imagesDir, _id);
                    return chapterContent;
                } else {
                    return "";
                }
            })
            const chapterContents = await Promise.all(chapterDownloadPromises)
            // 拼接所有章节的内容
            content += chapterContents.join("\n\n");
        }
        return content;
    }

    private static async ParseImg(
        content: string,
        imagesDir: string,
        chapId: number
    ) {
        const regex = /\[img=[^\]]*\](https:\/\/[^[]+)\[\/img\]/;
        const match = content.match(regex);
        if (match) {
            let url = match[1];
            if (url) {
                await this.imgDownload(String(chapId), imagesDir, url);
                content = content.replace(match[0], `![](imgs/${chapId}.webp)`);
            }
        }
        return content;
    }

    private static async imgDownload(name: string, imagesDir: string, url: string) {
        const imgPath = path.join(imagesDir, `${name}.webp`);
        const data = await SfacgClient.image(url);
        await fse.outputFile(imgPath, data);
    }

    static async Once(bookshelf: boolean = false) {
        const client = new SfacgClient();
        let books: any;
        if (!bookshelf) {
            const bookName = await question("请输入书名：");
            books = await client.searchInfos(bookName as string);
        }
        const { userName, passWord } = await questionAccount();
        await client.login(userName as string, passWord as string);
        if (bookshelf) {
            books = await client.bookshelfInfos();
        }
        const _novelId = books && (await this.selectBookFromList(books));
        const _save = await question("[1]是(默认)\n[2]否\n是否上传数据库：");
        if (_save !== "2") {
            // 数据库上传&&从数据库下载
            await this.UploadDB(_novelId, client.cookie as string);
            await this.DownLoad(_novelId)
        } else {
            // 用户直接下载
            client.cookie && await this.DownLoad(_novelId, client.cookie)
        }
    }

    /**### 待添加数据库重复提示
     * 将小说上传至数据库
     * @param novelId  小说ID
     * @param cookie  用户凭证
     * @param exclude  排除的章节数组
     */
    private static async UploadDB(novelId: number, cookie: string, exclude?: number[]) {
        const client = new SfacgClient();
        // 设置ck,拿已购章节
        client.cookie = cookie;
        const volumeInfos = await client.volumeInfos(novelId);
        volumeInfos &&
            volumeInfos.map((volumes) => {
                volumes.chapterList.map(async (chapter) => {
                    if (chapter.needFireMoney == 0) {
                        const content = await client.contentInfos(chapter.chapId);
                        content && (await _SfacgCache.UpsertChapterInfo({
                            chapId: chapter.chapId,
                            volumeId: volumes.volumeId,
                            ntitle: chapter.ntitle,
                            novelId: novelId,
                            content: content
                        }));
                    }
                });
            });
    }

    // 表格输出，稍作美化
    private static async selectBookFromList(
        books: IsearchInfos[]
    ): Promise<number> {
        const table = new Table({
            head: [
                colorize("序号", "blue"),
                colorize("书籍名称", "yellow"),
                colorize("作者", "green"),
                colorize("书籍ID", "purple"),
            ],
        });
        books.forEach((book, index) => {
            table.push([
                colorize(`${index + 1}`, "blue"),
                colorize(`${book.novelName}`, "yellow"),
                colorize(`${book.authorName}`, "green"),
                colorize(`${book.novelId}`, "purple"),
            ]);
        });
        console.log(table.toString());
        const index = await question(
            `请输入${colorize("[1]", "blue")}~${colorize(
                `[${books.length}]`,
                "blue"
            )}序号：`
        );
        return books[(index as number) - 1].novelId;
    }

    private static async markdownHead(novelInfo: InovelInfo) {
        return `---
title: ${novelInfo.novelName}
author: ${novelInfo.authorName}
lang: zh-Hans
description: |-
  ${novelInfo.intro}
cover-image: imgs/cover.webp    
---\n\n`;
    }
}


// (async () => {
//     const imagesDir = path.join(outputDir, "测试", "imgs");
//     await _SfacgDownloader.DownLoad(567122)
// })()
