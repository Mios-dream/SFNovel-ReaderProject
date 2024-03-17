import { SfacgAccountManager, SfacgClient } from "./client/Sfacg";
import fs from "fs-extra"

(async () => {
    // const sfacg = new SfacgClient()
    // await sfacg.login("13696458853", "dddd1111")
    // let a = await sfacg.getTasks()
    // fs.outputJsonSync("./TESTDATA/tasks.json",a)
    const a = new SfacgAccountManager()
    a.addAccount({
        userName: "11111", // 用户名
        passWord: "2222222" // 密码
    })

})()

