import { SfacgClient } from "./client/Sfacg/client";
import fs from "fs";
import * as dotenv from "dotenv";
import { Config } from "./client/config";
import { SfacgOption } from "./client/Sfacg/types/ITypes";
(async () => {
  dotenv.config();
  const SfacgOption: SfacgOption = Config.sfacg;
  // const CiweimaoOption: CiweimaoOption = Config.ciweimao
  const sfacg = new SfacgClient();

  // 小说信息
  const novelInfo = await sfacg.novelInfo(216187);
  fs.writeFileSync("./TESTDATA/novelInfo.json", JSON.stringify(novelInfo));

  // 登录状态
  const loginInfo = await sfacg.login("13696458853", "dddd1111", SfacgOption);
  fs.writeFileSync("./TESTDATA/loginInfo.json", JSON.stringify(loginInfo));

  // 章节列表
  const volumeInfos = await sfacg.volumeInfos(567122);
  fs.writeFileSync("./TESTDATA/volumeInfos.json", JSON.stringify(volumeInfos));

  // 书本内容
  const contentInfos = await sfacg.contentInfos(7431226);
  fs.writeFileSync(
    "./TESTDATA/contentInfos.json",
    JSON.stringify(contentInfos)
  );

  // 搜索小说信息
  const searchInfos = await sfacg.searchInfos("屠龙", 0, 10);
  fs.writeFileSync("./TESTDATA/searchInfos.json", JSON.stringify(searchInfos));

  // 我的书架信息
  const bookshelfInfos = await sfacg.bookshelfInfos();
  fs.writeFileSync(
    "./TESTDATA/bookshelfInfos.json",
    JSON.stringify(bookshelfInfos)
  );

  // 作品分类信息
  const typeInfo = await sfacg.typeInfo();
  fs.writeFileSync("./TESTDATA/typeInfo.json", JSON.stringify(typeInfo));

  // 所有标签信息
  const tags = await sfacg.tags();
  fs.writeFileSync("./TESTDATA/tags.json", JSON.stringify(tags));

  // 谷谷谷
  const novels = await sfacg.novels("0");
  fs.writeFileSync("./TESTDATA/novels.json", JSON.stringify(novels));

  // 图片测试
  const image = await sfacg.image(
    "https://rss.sfacg.com/web/novel/images/UploadPic/2023/02/3c1d9d6a-339a-43e5-a3bb-d1174bd3ea0e.jpg"
  );
  fs.writeFileSync("./TESTDATA/image.webp", image);

  // 看广告领代币次数
  const adBonusNum = await sfacg.adBonusNum();
  fs.writeFileSync("./TESTDATA/adBonusNum.json", JSON.stringify(adBonusNum));

  // 看广告领代币
  const adBonus = await sfacg.adBonus(adBonusNum);
  fs.writeFileSync("./TESTDATA/adBonus.json", JSON.stringify(adBonus));
})();
