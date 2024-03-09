import { SfacgClient } from "./client/Sfacg/client";
import fs from "fs";
(async () => {
  const sfacg = new SfacgClient();

  // 小说信息
  const novelInfo = await sfacg.novelInfo(216187);
  console.log(novelInfo);
  fs.writeFileSync("./TESTDATA/novelInfo.json", JSON.stringify(novelInfo));

  // 登录状态
  const loginStatus = await sfacg.login("13696458853", "dddd1111");
  console.log(loginStatus);
  fs.writeFileSync("./TESTDATA/loginStatus.json", JSON.stringify(loginStatus));

  // 用户信息
  const userInfo = await sfacg.userInfo();
  console.log(userInfo);
  fs.writeFileSync("./TESTDATA/userInfo.json", JSON.stringify(userInfo));

  // 章节列表
  const volumeInfos = await sfacg.volumeInfos(216187);
  console.log(volumeInfos);
  fs.writeFileSync("./TESTDATA/volumeInfos.json", JSON.stringify(volumeInfos));

  // 书本内容
  const contentInfos = await sfacg.contentInfos(2824824);
  console.log(contentInfos);
  fs.writeFileSync(
    "./TESTDATA/contentInfos.json",
    JSON.stringify(contentInfos)
  );

  // 搜索小说信息
  const searchInfos = await sfacg.searchInfos("屠龙", 0, 10);
  console.log(searchInfos);
  fs.writeFileSync("./TESTDATA/searchInfos.json", JSON.stringify(searchInfos));

  // 我的书架信息
  const bookshelfInfos = await sfacg.bookshelfInfos();
  console.log(bookshelfInfos);
  fs.writeFileSync(
    "./TESTDATA/bookshelfInfos.json",
    JSON.stringify(bookshelfInfos)
  );

  // 我的书架信息
  const categories = await sfacg.bookshelfInfos();
  console.log(categories);
  fs.writeFileSync("./TESTDATA/categories.json", JSON.stringify(categories));

  const tags = await sfacg.tags();
  console.log(tags);
  fs.writeFileSync("./TESTDATA/tags.json", JSON.stringify(tags));

  const novels = await sfacg.novels("0");
  console.log(novels);
  fs.writeFileSync("./TESTDATA/novels.json", JSON.stringify(novels));
})();
