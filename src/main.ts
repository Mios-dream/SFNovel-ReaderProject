import { Sfacg } from "./client/Sfacg/index";

const a = new Sfacg()
a.init()




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


