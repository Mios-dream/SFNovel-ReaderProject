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

## 音频下载

`AudioSrc` 是 MP3 地址，可使用普通 HTTP GET 下载；已验证响应为 `audio/mpeg`，并支持 `Range: bytes` 分段请求。音频 URL 是否需要 Cookie 可能因资源和时间变化，下载器应在请求时保留 Cookie，并处理 `401/403`。

## 相关 REST 接口

```http
GET https://api.sfacg.com/albums/{albumId}
GET https://api.sfacg.com/albums/{albumId}/chaps
```

`/albums/{albumId}` 可以返回专辑元数据和封面；`/chaps` 在当前验证中受接口权限/校验限制，不作为主要实现入口。网页 `getAudioInfo` 返回的 `VolumeSet[].AudioSet[]` 是目前更可靠的音频目录来源。
