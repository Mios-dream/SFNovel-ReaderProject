
import { SfacgClient } from "./client/Sfacg/api/client";
import { Sfacg } from "./client/Sfacg/index";
// import crypto from "crypto"
// import axios from "axios";
// import { v4 as uuidv4 } from "uuid";
(async () => {
    const a = new Sfacg()
    a.init()

})()





// function parseContent(chapId: number, content: string): { content: string, urls: string[] } {
//     // 创建一个正则表达式用来匹配图片URL
//     const imgRegExp = /\[img=[\d]+,[\d]+\](.*?)\[\/img\]/g;

//     // 创建一个数组存储找到的图片URL
//     let urls: string[] = [];

//     // 替换content中的图片URL，并将找到的URL添加到数组中
//     const newContent = content.replace(imgRegExp, (match, url) => {
//         urls.push(url);
//         return `![](${chapId}.n.webp)`;
//     });

//     // 返回替换后的content和找到的URL数组
//     return { content: newContent, urls };
// }

// // 测试函数
// const chapId = 21;
// const content = "\r\r\n[img=700,494]https://rss.sfacg.com/web/novel/images/UploadPic/2023/02/3c1d9d6a-339a-43e5-a3bb-d1174bd3ea0e.jpg[/img]\r\r\n夏鲁鲁与夏依\r\n感谢sf画师绘制";
// const result = parseContent(chapId, content);
// console.log(result);


// (async () => {
//     function sfSecurity(): string {
//         const uuid = uuidv4().toUpperCase();
//         const timestamp = Math.floor(Date.now() / 1000);
//         const data = `${uuid}${timestamp}${devicetoken}FMLxgOdsfxmN!Dt4`;
//         const hash = crypto
//             .createHash("md5")
//             .update(data)
//             .digest("hex")
//             .toUpperCase();
//         return `nonce=${uuid}&timestamp=${timestamp}&devicetoken=${devicetoken}&sign=${hash}`;
//     }
//     const devicetoken = "2C56A9FA-41C4-3001-8F82-0DEFE2E20773"
//     const response = await axios.post("https://api.sfacg.com//sessions", {
//         userName: "13696458853",
//         passWord: "dddd1111",
//     }, {
//         headers: {
//             authorization: "Basic YW5kcm9pZHVzZXI6MWEjJDUxLXl0Njk7KkFjdkBxeHE=",
//             "user-agent": `boluobao/4.9.98(android;34)/H5/${devicetoken}/H5`,
//             "sfsecurity": sfSecurity()
//         },
//     });
//     console.log(response.headers["set-cookie"]);

//     // 解析set-cookie数组，提取出cookie名称和值
//     const cookies = response.headers["set-cookie"]&&response.headers["set-cookie"].map(cookie => {
//         return cookie.split(';')[0]; // 只获取每个cookie的名称和值部分
//     }).join('; '); // 将所有cookie连接成一个字符串，用分号加空格隔开

//     // 打印出用于请求头的cookie字符串
//     console.log(cookies);
        
// }
// )()