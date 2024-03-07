import { SfacgClient } from "./client/sfacg/client"
import fs from "fs"
(async () => {
    const sfacg = new SfacgClient()

    // 小说信息
    const novelInfo = await sfacg.novelInfo(216187)
    console.log(novelInfo)
    fs.writeFileSync("./TESTDATA/novelInfo.json", JSON.stringify(novelInfo))

    // 登录状态
    const loginStatus = await sfacg.login("13696458853", "dddd1111")
    console.log(loginStatus)
    fs.writeFileSync("./TESTDATA/loginStatus.json", JSON.stringify(loginStatus))

    // 用户信息
    const userInfo = await sfacg.userInfo()
    console.log(userInfo)
    fs.writeFileSync("./TESTDATA/userInfo.json", JSON.stringify(userInfo))

    // 章节列表
    const volumeInfos = await sfacg.volumeInfos(216187)
    console.log(volumeInfos)
    fs.writeFileSync("./TESTDATA/volumeInfos.json", JSON.stringify(volumeInfos))
    
    // 书本内容
    const contentInfos = await sfacg.contentInfos("2824824")
    console.log(contentInfos)
    fs.writeFileSync("./TESTDATA/contentInfos.json", JSON.stringify(contentInfos))

    console.log(sfacg.sfSecurity())


})()

