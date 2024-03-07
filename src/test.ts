import { SfacgClient } from "./client/sfacg/client"


(async ()=> {
    const sfacg = new SfacgClient()
    let a = await sfacg.login("13696458853", "dddd1111")
    let b = await sfacg.userInfo()
    console.log(a)
})()