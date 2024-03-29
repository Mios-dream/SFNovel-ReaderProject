import { SfacgClient } from "../api/client";
import { Ichapter, InovelInfo, IsearchInfos, IvolumeInfos } from "../types/ITypes";
import { _SfacgCache } from "./cache";
import fse from 'fs-extra';
import path from "path"
import Table from "cli-table3";
import { colorize, question, questionAccount } from "../../../utils/tools";

const outputDir = path.join(process.cwd(), 'output', '菠萝包轻小说');

export class _SfacgDownloader {


    /**
     * 如果有cookie则从用户下载，否则从数据库下载
     * @param novelId 
     * @param cookie 
     */
    static async DownLoad(novelId: number, cookie?: string) {
        const _client = new SfacgClient()
        const _novelInfo = await _client.novelInfo(novelId)
        _client.cookie = cookie
        const _volumeInfos = await _client.volumeInfos(novelId)
        // 初始化路径啥的
        if (_novelInfo && _volumeInfos) {
            // 路径定义和封面下载
            const MarkDownHead = await this.markdownHead(_novelInfo)
            const imagesDir = path.join(outputDir, _novelInfo.novelName, 'imgs');
            const novelPath = path.join(outputDir, _novelInfo.novelName, `${_novelInfo.novelName}.md`);
            await fse.ensureDir(imagesDir);
            await this.imgDownload("cover.webp", imagesDir, _novelInfo.novelCover)
            // 正文部分处理，有ck就从Sf轻小说下载（volume中已火币变为0的），否则从数据库
            let content: string
            _volumeInfos.map(_volumeInfo => {
                content += "# " + _volumeInfo.title
                _volumeInfo.chapterList.map(async _chapter => {
                    let _oldContent: any
                    if (cookie && _chapter.needFireMoney === 0) {
                        _oldContent = await _client.contentInfos(_chapter.chapId)
                    } else {
                        _SfacgCache.GetChapterWithContentByVolumeId(_volumeInfo.volumeId)
                    }
                    content += _oldContent && await this.ParseImg(_oldContent, imagesDir, _chapter.chapId)
                })
            })
        }
    }

    private static async ParseImg(content: string, imagesDir: string, chapId: number) {
        const regex = /\[img=[^\]]*\](https:\/\/[^[]+)\[\/img\]/;
        const match = content.match(regex);
        if (match) {
            let url = match[1]
            if (url) {
                await this.imgDownload(chapId, imagesDir, url)
                content = content.replace(match[0], `![](imgs/${chapId}.webp)`)
            }
        }
        return content
    }

    private static async imgDownload(name: any, imagesDir: string, url: string) {
        const imgPath = path.join(imagesDir, name)
        const data = await SfacgClient.image(url)
        fse.outputFileSync(imgPath, data)
    }

    static async Once(bookshelf: boolean = false) {
        const client = new SfacgClient()
        let books: any
        if (!bookshelf) {
            const bookName = await question("请输入书名：");
            books = await client.searchInfos(bookName as string);
        }
        const { userName, passWord } = await questionAccount()
        await client.login(userName as string, passWord as string)
        if (bookshelf) {
            books = await client.bookshelfInfos()
        }
        const _novelId = books && await this.selectBookFromList(books)
        const _save = await question("[默认]是\n[1]否\n是否上传数据库：");
        if (_save === "1") {
            await _SfacgCache.UpdateNovelInfo(_novelId)
            this.UploadDB(_novelId, client.cookie as string)
            const _down = await question("[默认]是\n[1]否\n是否从数据库下载：");
            //     if (down === "1") {
            //     } else {
            //     }
        }
    }



    /**
     * 将小说上传至数据库
     * @param novelId  小说ID
     * @param cookie  用户凭证
     */
    private static async UploadDB(novelId: number, cookie: string) {
        const client = new SfacgClient()
        // 设置ck,拿已购章节
        client.cookie = cookie
        const volumeInfos = await client.volumeInfos(novelId)
        volumeInfos && volumeInfos.map(volumes => {
            volumes.chapterList.map(async chapter => {
                if (chapter.needFireMoney == 0) {
                    const content = await client.contentInfos(chapter.chapId)
                    content && await _SfacgCache.UpdatechapterContent(chapter.chapId, content)
                }
            })
        })
    }

    // 表格输出，稍作美化
    private static async selectBookFromList(books: IsearchInfos[]): Promise<number> {
        const table = new Table({
            head: [colorize('序号', "blue"), colorize('书籍名称', "yellow"), colorize('作者', "green"), colorize('书籍ID', "purple")],
        })
        books.forEach((book, index) => {
            table.push([
                colorize(`${index + 1}`, "blue"),
                colorize(`${book.novelName}`, "yellow"),
                colorize(`${book.authorName}`, "green"),
                colorize(`${book.novelId}`, "purple"),
            ]);
        })
        console.log(table.toString());
        const index = await question(`请输入${colorize("[1]", "blue")}~${colorize(`[${books.length}]`, "blue")}序号：`);
        return books[index as number - 1].novelId
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

