import { SfacgClient } from "../api/client";
import { Ichapter, InovelInfo, IsearchInfos } from "../types/ITypes";
import { _SfacgCache } from "./cache";
import fse from 'fs-extra';
import path from "path"
import Table from "cli-table3";
import { colorize, question } from "../../../utils/tools";

const outputDir = path.join(process.cwd(), 'output', '菠萝包轻小说');
const imagesDir = path.join(outputDir, 'imgs');
const coverImagePath = path.join(imagesDir, 'cover.webp');
export class _SfacgDownloader {


    static async DownLoadFromDB(novelId: number) {
        const client = new SfacgClient()
        const novelInfo = await client.novelInfo(novelId)
        const volumeInfos = await client.volumeInfos(novelId)
        await fse.ensureDir(imagesDir);
        let markdownContent = await this.markdownHead(novelInfo as InovelInfo)
        const cover = novelInfo && await SfacgClient.image(novelInfo.novelCover)
        fse.outputFileSync(coverImagePath, cover)
        // 循环每卷
        if (volumeInfos && novelInfo) {

            for (const volume of volumeInfos) {
                markdownContent += `# ${volume.title}\n\n`;
                const data = await _SfacgCache.GetChapterFromVolumeId(volume.volumeId)
                if (data)
                    for (const chapter of data as Ichapter[]) {
                        console.log(chapter.chapId);
                        if (chapter.content) {
                            markdownContent += `## ${chapter.ntitle}\n\n`;
                            markdownContent += `${chapter.content}\n\n`;
                        }
                    }
            }
            const novelMarkdownPath = path.join(outputDir, `${novelInfo?.novelName.replace(/\s+/g, '_')}.md`);
            markdownContent && fse.outputFileSync(novelMarkdownPath, markdownContent)
        }
    }


    static async OnceDownload(bookshelf: boolean = false) {
        const client = new SfacgClient()

        let books: any, novelId: any
        if (!bookshelf) {
            const bookName = await question("请输入书名：");
            books = await client.searchInfos(bookName as string);

        }
        const userName = await question("输入账号：");
        const passWord = await question("输入密码：");
        await client.login(userName as string, passWord as string)
        if (bookshelf) {
            books = await client.bookshelfInfos()
        }
        novelId = books && await this.selectBookFromList(books)
        const results = await _SfacgCache.GetChapterNoContent(novelId)
        const volumeInfos = await client.volumeInfos(novelId)
        const ordered: number[] = volumeInfos
            ? volumeInfos.flatMap(volume =>
                volume.chapterList.filter(chapter => chapter.needFireMoney === 0).map(chapter => chapter.chapId))
            : [];
        results && results.map(async (result) => {
            if (ordered.includes(result.chapId)) {
                const content = await client.contentInfos(result.chapId)
                content && await _SfacgCache.UpdatechapterContent(result.chapId, content)
            }
        })
    }


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
cover-image: ${novelInfo.novelCover}
---\n\n`;
    }
}

