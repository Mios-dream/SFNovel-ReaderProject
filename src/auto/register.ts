import { Sfacg } from "../client/Sfacg";

// 独立运行时执行一次账号注册流程，供旧版命令行工具使用。
(async () => {
    const sfacg = new Sfacg()
    await sfacg.Regist()
})()
