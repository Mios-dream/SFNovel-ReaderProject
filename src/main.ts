import { SfacgClient } from "./client/Sfacg/client";
import fs from "fs-extra"
import Config from "./utils/config";
import { SfacgHttp } from "./client/Sfacg/http";
(async () => {
  // const CiweimaoOption: CiweimaoOption = Config.ciweimao
  const SfacgOption = Config.sfacg;
  const sfacg = new SfacgClient();
  // 搜索小说信息
  const searchInfos = await sfacg.searchInfos("屠龙", 0, 10);
  await fs.outputJson("./TESTDATA/searchInfos.json", searchInfos);

  // 小说信息
  const novelInfo = await sfacg.novelInfo(216187);
  await fs.outputJson("./TESTDATA/novelInfo.json", novelInfo);

  // 作品分类信息
  const typeInfo = await sfacg.typeInfo();
  await fs.outputJson("./TESTDATA/typeInfo.json", typeInfo);

  // 所有标签信息
  const tags = await sfacg.tags();
  await fs.outputJson("./TESTDATA/tags.json", tags);

  // 章节列表
  const volumeInfos = await sfacg.volumeInfos(567122);
  await fs.outputJson("./TESTDATA/volumeInfos.json", volumeInfos);

  // 登录状态
  const loginInfo = await sfacg.login("13696458853", "dddd1111", SfacgOption);
  await fs.outputJson("./TESTDATA/loginInfo.json", loginInfo);

  // 章节内容
  const contentInfos = await sfacg.contentInfos(7431226);
  await fs.outputJson("./TESTDATA/contentInfos.json", contentInfos);

  // 我的书架信息
  const bookshelfInfos = await sfacg.bookshelfInfos();
  await fs.outputJson("./TESTDATA/bookshelfInfos.json", bookshelfInfos);

  // 谷谷谷
  const novels = await sfacg.novels("0");
  await fs.outputJson("./TESTDATA/novels.json", novels);

  // 图片测试
  const image = await sfacg.image(
    "https://rss.sfacg.com/web/novel/images/UploadPic/2023/02/3c1d9d6a-339a-43e5-a3bb-d1174bd3ea0e.jpg"
  );
  await fs.outputFile("./TESTDATA/image.webp", image);

  // 看广告领代币次数
  const adBonusNum = await sfacg.adBonusNum();
  await fs.outputJson("./TESTDATA/adBonusNum.json", adBonusNum);

  // 看广告领代币
  const adBonus = await sfacg.adBonus();
  await fs.outputJson("./TESTDATA/adBonus.json", adBonus);

  // 签到奖励
  const signBous = await sfacg.newSign();
  await fs.outputJson("./TESTDATA/signBous.json", signBous);

})();
