import { Sfacg } from "./client/Sfacg";
import { SfacgAccountManager, SfacgClient } from "./client/Sfacg/client";
import fs from "fs-extra"

(async () => {
    // const sfacg = new SfacgClient()
    // await sfacg.login("13696458853", "dddd1111")

    const sfacg = new Sfacg()
    sfacg.init()

    // let a = await sfacg.adBonus(21)
    // console.log(a)
    // fs.outputJsonSync("./TESTDATA/adBonus.json", a)

    // let a = await sfacg.claimTask(21)
    // console.log(a)
    // fs.outputJsonSync("./TESTDATA/claimTask.json", a)

    // let a = await sfacg.adBonus(21)
    // let b = await sfacg.adBonusNum()
    // console.log(a)
    // console.log(b)
    // fs.outputJsonSync("./TESTDATA/adBonus.json", a)
    // fs.outputJsonSync("./TESTDATA/adBonusNum.json", b)
    // const a = new SfacgAccountManager()
    // a.addAccount({ 
    //     userName: "11111", // 用户名
    //     passWord: "2222222" // 密码
    // })

    // Config.Sfacg .proxy = "14561"
})()

