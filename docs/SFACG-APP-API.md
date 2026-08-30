# SFACG App API 登录与签名

> 验证日期：2026-08-30。本文只记录登录和请求签名协议。该协议可能被 SF 随时调整，不代表稳定的公开 API。

## 已验证的登录方法

当前可用的是 App API 请求画像：

- 地址：`https://api.sfacg.com`
- 方法：`POST /sessions`
- Basic Auth：固定的 App 基础认证
- `Accept`：`application/vnd.sfacg.api+json;version=1`
- `Content-Type`：`application/json; charset=UTF-8`
- `User-Agent`：`boluobao/5.2.16(android;35)/OPPO/{deviceToken 小写}/OPPO`
- `deviceToken`：大写 UUID；实测固定值 `910D166A-736E-3231-8B21-8D12DFD75F16` 可用
- `salt`：`lPQDb9AKO7$LjkPG`
- 时间戳：Unix 毫秒时间戳

登录体使用小写字段名：

```json
{
  "username": "账号",
  "password": "密码",
  "shuMeiId": ""
}
```

成功条件是 HTTP 状态码为 `200` 且响应中的 `status.httpCode` 为 `200`。从响应的 `Set-Cookie` 中提取 `.SFCommunity` 和 `session_APP`，后续请求通过 `Cookie` 发送。密码和 Cookie 只能保留在内存或受控会话存储中，不得写入日志、源码、配置或 Git。

## 签名逻辑

每个请求都需要发送以下格式的 `SFSecurity` 头：

```text
nonce={大写 UUID v4}&timestamp={Unix 毫秒}&devicetoken={deviceToken}&sign={大写 MD5}
```

`sign` 的计算步骤：

1. 将 `timestamp + salt + deviceToken + nonce` 按 ASCII 编码为 `authString`，长度为 101 字节。
2. 将 `nonce` 重复四次。对重复串第 2、3、4、5 个字节分别计算 `byte - floor(byte / 0x24) * 0x24`，得到四个偏移量。
3. 从重复串按四个偏移量分别截取长度 `13`、`16`、`36`、`36` 的片段，拼接为 `nonceReorder`。
4. 对 `authString` 与 `nonceReorder` 的每个字节执行 `(a + b) >> 1`，得到 101 个字符。
5. 将结果按 `D + A + C + B` 重排，其中四段长度依次为 `13`、`16`、`36`、`36`。
6. 对字符执行上游的 ASCII 归一化规则：小于 `0x30` 或位于数字/大小写字母间隙的字符加 `19`，`0x39` 间隙取 `0x39`。
7. 对归一化后的 UTF-8 字符串计算 MD5，并转为大写十六进制。

不要使用简单的 `MD5(nonce + timestamp + deviceToken + salt)` 替代上述算法。实测中，简单算法会得到 `417/782`。

## Nonce 流程

复杂签名必须先用章节接口预检 nonce：

```text
生成 nonce
  -> GET /Chaps/8436696?expand=content%2Cexpand.content
  -> status.httpCode == 417：更换 nonce，有限次重试
  -> 非 417：保留 nonce
  -> 用同一个 nonce 请求 /sessions 和后续 API
```

不能在登录后为每个请求重新生成 nonce；实测会使后续请求再次返回 `417/782`。

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
const SALT = "lPQDb9AKO7$LjkPG";
const BASIC_AUTH = "Basic YW5kcm9pZHVzZXI6MWEjJDUxLXl0Njk7KkFjdkBxeHE=";
const USERNAME = process.env.SFACG_USERNAME;
const PASSWORD = process.env.SFACG_PASSWORD;

function sign(nonce: string, timestamp: number): string {
  const repeated = Buffer.from(nonce.repeat(4), "ascii");
  const offset = (index: number) => {
    const value = repeated[index];
    return value - Math.floor(value / 0x24) * 0x24;
  };
  const reorderedNonce = Buffer.concat([
    repeated.subarray(offset(1), offset(1) + 13),
    repeated.subarray(offset(2), offset(2) + 16),
    repeated.subarray(offset(3), offset(3) + 36),
    repeated.subarray(offset(4), offset(4) + 36),
  ]);
  const auth = Buffer.from(`${timestamp}${SALT}${DEVICE_TOKEN}${nonce}`, "ascii");
  let mixed = "";
  for (let i = 0; i < 101; i += 1) {
    mixed += String.fromCharCode((auth[i] + reorderedNonce[i]) >> 1);
  }
  const result = `${mixed.slice(65)}${mixed.slice(0, 13)}${mixed.slice(29, 65)}${mixed.slice(13, 29)}`;
  let normalized = "";
  for (const character of result) {
    const code = character.charCodeAt(0);
    if (code < 0x30) {
      normalized += 0x39 < code + 19 && code + 19 < 0x41
        ? String.fromCharCode(0x39)
        : String.fromCharCode(code + 19);
    } else if ((0x39 < code && code < 0x41) || (0x5a < code && code < 0x61)) {
      normalized += String.fromCharCode(code + 19);
    } else {
      normalized += character;
    }
  }
  return crypto.createHash("md5").update(normalized, "utf8").digest("hex").toUpperCase();
}

function requestHeaders(nonce: string) {
  const timestamp = Date.now();
  return {
    Authorization: BASIC_AUTH,
    Accept: "application/vnd.sfacg.api+json;version=1",
    "Accept-Charset": "UTF-8",
    "Content-Type": "application/json; charset=UTF-8",
    "User-Agent": `boluobao/5.2.16(android;35)/OPPO/${DEVICE_TOKEN.toLowerCase()}/OPPO`,
    "Accept-Encoding": "gzip",
    SFSecurity: `nonce=${nonce}&timestamp=${timestamp}&devicetoken=${DEVICE_TOKEN}&sign=${sign(nonce, timestamp)}`,
  };
}

async function main() {
  if (!USERNAME || !PASSWORD) throw new Error("Set SFACG_USERNAME and SFACG_PASSWORD first.");
  let nonce = "";
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const candidate = uuidv4().toUpperCase();
    const probe = await axios.get(
      `${HOST}/Chaps/8436696?expand=content%2Cexpand.content`,
      { headers: requestHeaders(candidate), validateStatus: () => true },
    );
    if (probe.data?.status?.httpCode !== 417) {
      nonce = candidate;
      break;
    }
  }
  if (!nonce) throw new Error("No usable nonce returned by chapter probe.");

  const login = await axios.post(
    `${HOST}/sessions`,
    { username: USERNAME, password: PASSWORD, shuMeiId: "" },
    { headers: requestHeaders(nonce), validateStatus: () => true },
  );
  const status = login.data?.status;
  const cookie = (Array.isArray(login.headers["set-cookie"]) ? login.headers["set-cookie"] : [])
    .map((value) => value.split(";", 1)[0])
    .filter((value) => /^(\.SFCommunity|session_APP)=/.test(value))
    .join("; ");
  let userHttpCode: number | undefined;
  if (status?.httpCode === 200 && cookie) {
    const user = await axios.get(`${HOST}/user`, {
      headers: { ...requestHeaders(nonce), Cookie: cookie },
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
