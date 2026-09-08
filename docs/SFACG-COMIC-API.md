# SFACG 漫画接口验证

验证日期：2026-09-04。漫画详情使用匿名 App API；SF 未提供与小说 `GET /novels/{id}/dirs` 对等的漫画目录 App API，因此目录和图片仍使用漫画站公开接口。

## 验证样本

搜索关键词：`血姬与骑士`

| 类型 | 标识 | 验证结果 |
| --- | --- | --- |
| 小说 | `155640` | 搜索响应的 `novels[]` 返回该作品 |
| 有声 | `155640` | 搜索响应的 `albums[]` 返回该作品 |
| 漫画 | `2937` | 搜索响应的 `comics[]` 返回该作品 |

## 已接入接口

1. `GET https://api.sfacg.com/search/novels/result/new?q=...`：搜索响应同时含 `novels`、`albums`、`comics`。
2. `GET https://api.sfacg.com/comics/{comicId}`：返回漫画详情及 `folderName` 网页目录映射。样本 `2937` 返回 `folderName: XJYQS`。
3. `GET https://manhua.sfacg.com/mh/{folderName}/`：返回漫画章节链接，例如 `/mh/XJYQS/81461/`。
4. `GET https://manhua.sfacg.com/ajax/Common.ashx?op=getPics&cid={comicId}&chapId={chapterId}&serial={fn}&path={nv}`：从章节页内的 `c`、`chapId`、`fn`、`nv` 取得参数后，返回该章节的 HTTPS 图片列表。样本“序章”章节 `81461` 返回 37 张图片。

章节页和图片接口必须带同源 Referer；所有请求都在 Rust 原生层执行，图片链接不会返回给渲染进程。

## VIP 章节与会话

`GET /comics/{comicId}` 当前只返回漫画作品元数据（例如 `comicName`、`folderName`、`comicCover`、`signStatus`），不返回章节列表或逐章 `has/isUnlocked`。漫画网页目录也只稳定提供章节 ID、标题和 VIP 标记。因此不能把登录 Cookie 等同于章节已购买；只有目录节点明确提供结构化拥有标记时才标记为已解锁，否则保持锁定，避免错误下载。

未登录时 VIP 章节保持锁定。登录后可选择并下载，平台仍会按账户购买权限响应；若接口拒绝，任务会显示脱敏的上游错误，不会写入完成标记。接口结构可能随 SF 调整而变化。
