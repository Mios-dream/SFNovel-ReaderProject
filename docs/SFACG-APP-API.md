# SFACG App API 登录、签名与匿名内容接口

> 匿名接口实测日期：2026-09-06。本文记录登录、请求签名及无登录会话的内容接口。该协议可能被 SF 随时调整，不代表稳定的公开 API。

## 已验证的登录方法

当前可用的是 App API 请求画像：

- 地址：`https://api.sfacg.com`
- 方法：`POST /sessions`
- Basic Auth：固定的 App 基础认证
- `Accept`：`application/vnd.sfacg.api+json;version=1`
- `Content-Type`：`application/json; charset=UTF-8`
- `User-Agent`：`boluobao/5.0.36(android;34)/H5/{deviceToken}/H5`
- `deviceToken`：每个安装稳定保存的大写 UUID
- `salt`：`FN_Q29XHVmfV3mYX`
- 时间戳：Unix 秒级时间戳

登录体使用 App 客户端字段名：

```json
{
  "userName": "账号",
  "passWord": "密码"
}
```

成功条件是 HTTP 状态码为 `200` 且响应中的 `status.httpCode` 为 `200`。从响应的 `Set-Cookie` 中提取 `.SFCommunity` 和 `session_APP`，后续请求通过 `Cookie` 发送。密码和 Cookie 只能保留在内存或受控会话存储中，不得写入日志、源码、配置或 Git。

## 签名逻辑

每个请求都需要发送以下格式的 `SFSecurity` 头：

```text
nonce={大写 UUID v4}&timestamp={Unix 秒}&devicetoken={deviceToken}&sign={大写 MD5}
```

`sign` 的计算步骤：

1. 生成大写 UUID nonce。
2. 拼接 `nonce + timestamp + deviceToken + salt`。
3. 对拼接结果计算 MD5，并转为大写十六进制。

其中 `timestamp` 为 Unix 秒级时间戳。请求签名与 Basic Auth 是 App 协议字段，并不等于用户登录；下文“匿名”均表示请求不发送 `.SFCommunity`、`session_APP` 或其他 Cookie。

## 已验证的匿名内容接口

以下结果使用当前 Rust `AppClient` 的请求画像，且明确省略 `Cookie` 请求头；每个目标只请求一次。验证样本为小说 `155640`、漫画 `2937`、有声专辑 `137`、章节 `8436696`。记录只保留路由、响应状态和响应是否含数据。

| 路由 | HTTP / 业务状态 | 是否有 `data` | 当前用途与处理 |
| --- | --- | --- | --- |
| `GET /search/novels/result/new` | `200 / 200` | 是 | 搜索小说、有声与漫画。匿名 App 请求。 |
| `GET /novels/{novelId}` | `200 / 200` | 是 | 小说详情、封面和下载元数据。匿名 App 请求。 |
| `GET /novels/{novelId}/dirs` | `200 / 200` | 是 | 文字小说分卷与章节目录。匿名 App 请求。 |
| `GET /albums/{albumId}` | `200 / 200` | 是 | 有声专辑详情、封面和补充元数据。匿名 App 请求。 |
| `GET /comics/{comicId}` | `200 / 200` | 是 | 漫画名称及网页目录 `folderName` 映射。匿名 App 请求。 |
| `GET /Chaps/{chapterId}` | `401 / 401`，`errorCode 507` | 否 | 不可匿名。正文继续网页优先；仅在用户显式启用回退且具备 App 会话时使用 App。 |
| `GET /albums/{albumId}/chaps` | `401 / 401`，`errorCode 507` | 否 | 不可匿名，且不作为有声目录来源；继续使用官方网页 AJAX。 |

实现约束：所有表中已验证的公开接口必须通过 `AppClient::get_public_data` 请求。该方法仍发送协议所需的 Basic Auth、`SFSecurity` 与安装级设备标识，但强制不携带 App 会话。不要把“匿名 App 内容接口”扩展为账户、书架、正文、购买或设备上报接口，也不要根据一次成功推断 VIP 章节访问权限。

## Nonce 流程

每个请求独立生成 nonce，不需要章节接口预检，也不跨请求复用：

```text
生成新的 nonce
  -> 计算当前请求的 SFSecurity
  -> 发送 /sessions 或其他 App API
```

## 可复现登录代码

以下代码块是独立的 TypeScript/Node.js 示例，只依赖 `axios` 和 `uuid`。它不会输出密码、Cookie、签名或正文。运行前在当前进程设置环境变量，不要把真实值写进文件：

```powershell
$env:SFACG_USERNAME = "<账号>"
$env:SFACG_PASSWORD = "<密码>"
npx tsx sfacg-login.ts
```

```ts
import axios from "axios";
import crypto from "node:crypto";
import { v4 as uuidv4 } from "uuid";

const HOST = "https://api.sfacg.com";
const DEVICE_TOKEN = "910D166A-736E-3231-8B21-8D12DFD75F16";
const SALT = "FN_Q29XHVmfV3mYX";
const BASIC_AUTH = "Basic YW5kcm9pZHVzZXI6MWEjJDUxLXl0Njk7KkFjdkBxeHE=";
const USERNAME = process.env.SFACG_USERNAME;
const PASSWORD = process.env.SFACG_PASSWORD;

function sign(nonce: string, timestamp: number): string {
  return crypto.createHash("md5")
    .update(`${nonce}${timestamp}${DEVICE_TOKEN}${SALT}`, "utf8")
    .digest("hex")
    .toUpperCase();
}

function requestHeaders(nonce: string) {
  const timestamp = Math.floor(Date.now() / 1000);
  return {
    Authorization: BASIC_AUTH,
    Accept: "application/vnd.sfacg.api+json;version=1",
    "Accept-Charset": "UTF-8",
    "Content-Type": "application/json; charset=UTF-8",
    "User-Agent": `boluobao/5.0.36(android;34)/H5/${DEVICE_TOKEN}/H5`,
    "Accept-Encoding": "gzip",
    SFSecurity: `nonce=${nonce}&timestamp=${timestamp}&devicetoken=${DEVICE_TOKEN}&sign=${sign(nonce, timestamp)}`,
  };
}

async function main() {
  if (!USERNAME || !PASSWORD) throw new Error("Set SFACG_USERNAME and SFACG_PASSWORD first.");
  const login = await axios.post(
    `${HOST}/sessions`,
    { userName: USERNAME, passWord: PASSWORD },
    { headers: requestHeaders(uuidv4().toUpperCase()), validateStatus: () => true },
  );
  const status = login.data?.status;
  const cookie = (Array.isArray(login.headers["set-cookie"]) ? login.headers["set-cookie"] : [])
    .map((value) => value.split(";", 1)[0])
    .filter((value) => /^(\.SFCommunity|session_APP)=/.test(value))
    .join("; ");
  let userHttpCode: number | undefined;
  if (status?.httpCode === 200 && cookie) {
    const user = await axios.get(`${HOST}/user`, {
      headers: { ...requestHeaders(uuidv4().toUpperCase()), Cookie: cookie },
      validateStatus: () => true,
    });
    userHttpCode = user.data?.status?.httpCode;
  }
  console.log(JSON.stringify({
    httpStatus: login.status,
    httpCode: status?.httpCode,
    errorCode: status?.errorCode,
    message: status?.msg,
    cookieReceived: Boolean(cookie),
    userHttpCode,
  }));
}

void main().catch((error: any) => {
  console.error(JSON.stringify({
    code: error?.code,
    message: error?.message || "request failed",
    httpStatus: error?.response?.status,
    httpCode: error?.response?.data?.status?.httpCode,
    errorCode: error?.response?.data?.status?.errorCode,
  }));
  process.exitCode = 1;
});
```

## 安全与失效处理

- `417/782`：签名、时间戳单位、salt、请求头或 nonce 生命周期不匹配。
- `401/507`：请求未被当前 API 权限画像接受，不能直接判断为密码错误。
- `403`：会话已登录但资源无权限、未订阅或需付费。
- Cookie 等同于登录凭证，不得提交到 Git、日志或公开 issue。
- 本项目默认仍使用官方网页登录获取 Cookie；App API 方案应在低频、串行请求下单独验证后再接入生产代码。

## 正文字符恢复

App API 的 `data.content` 可能把汉字替换成另一组汉字。它不是需要解密的密文，而是逐字符的一一替换混淆。本项目内置了从 [Oevani/Sfacg_Downloader](https://github.com/Oevani/Sfacg_Downloader) 整理的 3751 项初始表：

```ts
const decoded = [...apiContent]
  .map((character) => dictionary[character] || character)
  .join("");
```

只对 API 来源正文执行替换，网页来源已经是正常文本，不能再次套用字典。设置中的“手动更新字典”会读取指定的公开章节，使用章节 API 返回的 `novelId/volumeId` 请求对应网页，删除非汉字后按位置建立新映射；两侧汉字数量不同或已有映射冲突时拒绝保存。成功后表保存在项目根目录的 `sfacg-content-dictionary.json`（已加入 Git 忽略），后续下载自动使用。
