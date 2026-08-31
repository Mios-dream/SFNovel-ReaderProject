# SF 有声小说接口说明

## 已验证接口

```http
GET https://i.sfacg.com/ajax/ashx/Common.ashx?op=getAudioInfo&nid={novelId}
```

`nid` 是小说 ID，不是有声专辑 ID。接口返回该小说关联的有声目录和音频地址。

成功响应的主要结构：

```json
{
  "status": 200,
  "data": {
    "NovelID": 567122,
    "NovelName": "...",
    "LastListenChapID": 0,
    "VolumeSet": [
      {
        "VolumeName": "...",
        "AudioSet": [
          {
            "AudioID": 80844,
            "ChapterID": 0,
            "VolumeIndex": 0,
            "ChapterTitle": "...",
            "AudioSrc": "https://rss.sfacg.com/web/audio/files/{albumId}/{uuid}.mp3"
          }
        ]
      }
    ]
  }
}
```

## 鉴权与请求头

该接口使用 SF 官网会话 Cookie。请求至少应携带登录后的 `Cookie`，建议同时携带：

```http
User-Agent: 浏览器 User-Agent
Accept: application/json, text/javascript, */*; q=0.01
X-Requested-With: XMLHttpRequest
Referer: https://i.sfacg.com/consume/book/
Cookie: .SFCommunity=...; session_PC=...
```

官方登录页：<https://passport.sfacg.com/Login.aspx>

官方网页登录由官方页面提交：

```http
POST https://passport.sfacg.com/Ajax/QuickLogin.ashx
Content-Type: application/x-www-form-urlencoded; charset=UTF-8
Referer: https://passport.sfacg.com/Login.aspx
X-Requested-With: XMLHttpRequest

name={账号}&password={密码}&al=false&ticket={腾讯滑块票据}&randstr={腾讯滑块随机串}
```

`ticket` 和 `randstr` 必须由官方腾讯滑块组件 `TCaptcha.js` 在用户完成验证后产生，不能用用户名和密码替代，也不应绕过验证。

本项目不再转发这两个字段。点击“打开官方登录窗口”后，应用会在平台原生 WebView 中打开官方登录页；账号、密码、滑块验证和 `QuickLogin.ashx` 提交均在同一官方浏览器上下文内完成。登录成功后，应用仅从该受控窗口读取 SF 会话 Cookie，用于本地下载。

## 本地会话保存

应用只保存 SF 签发的会话 Cookie，不保存账号密码。Windows 将 `用户名 + 会话 Cookie` 写入应用数据目录中的 `auth-session.bin`，并使用当前 Windows 用户的 DPAPI 加密；应用启动时自动解密恢复。Android 使用应用私有的 WebView `CookieManager` 持久化 Cookie，并在启动时同步到原生请求层。两端的密码登录和官方网页登录都会保存会话。

点击应用中的“退出当前会话”会同时清除内存会话和平台持久化会话。SF 官方会话本身可能过期，发生 `401/403` 时需要重新登录。

## 音频下载

`AudioSrc` 是 MP3 地址，可使用普通 HTTP GET 下载；已验证响应为 `audio/mpeg`，并支持 `Range: bytes` 分段请求。音频 URL 是否需要 Cookie 可能因资源和时间变化，下载器应在请求时保留 Cookie，并处理 `401/403`。

## 相关 REST 接口

```http
GET https://api.sfacg.com/albums/{albumId}
GET https://api.sfacg.com/albums/{albumId}/chaps
```

`/albums/{albumId}` 可以返回专辑元数据和封面；`/chaps` 在当前验证中受接口权限/校验限制，不作为主要实现入口。网页 `getAudioInfo` 返回的 `VolumeSet[].AudioSet[]` 是目前更可靠的音频目录来源。

## 安全注意事项

- Cookie 等同于登录凭证，不要提交到 Git、日志或公开 issue。
- 本项目不将 Cookie 写入源码、项目配置或日志；Windows 使用当前用户 DPAPI 加密，Android 使用应用私有 WebView Cookie 存储。
- 使用完毕后应退出 SF 账号或修改密码，使已暴露的旧 Cookie 失效。
- 仅下载自己有权访问和保存的内容，并遵守 SF 平台条款及版权要求。
