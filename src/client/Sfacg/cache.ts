import path from "path"

export class SfacgCache {

    SaveBookPath: string = path.resolve(__dirname, `../output/Accounts/Sfacg`)

    // 数据库中找到该书，若无则创建空表，返回未有章节id
    findBook(bookId: string) {

    }

    saveBook(bookId: number, bookName: string) {

    }

    replaceImage(chapId: number, content: string) {

    }

    jsonToTxt() {

    }

    txtToEpub() {

    }

}