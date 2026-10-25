# Android 真机调试记录

## 问题：真机无法连接 Vite 开发服务器

### 现象

- Android 调试包启动后显示空白页。
- WebView 报错：`error sending request for url (http://<开发机局域网 IP>:1420/)`。
- 手机使用移动网络、与开发机不在同一局域网，或开发机防火墙阻止端口时，均可能复现。
- 即使手机通过 USB 连接并能被 `adb devices` 识别，应用仍可能访问不到开发机的局域网地址。

### 原因

`tauri android dev` 默认会把调试包的 `build.devUrl` 改写为开发机局域网地址。USB 连接本身不会让手机上的应用自动访问开发机端口，因此当手机与开发机不处于同一网络时，WebView 会直接加载失败。

## 解决方法：adb reverse + localhost 开发地址

通过 USB 调试时，推荐把开发服务器地址切换为 `127.0.0.1`，并使用 `adb reverse` 将手机端口转发到开发机。

1. 确认设备已连接：

   ```bash
   adb devices
   ```

2. 转发页面端口和 HMR 端口：

   ```bash
   adb reverse tcp:1420 tcp:1420
   adb reverse tcp:1421 tcp:1421
   ```

3. 使用 localhost 地址启动 Android 开发模式：

   ```bash
   npm run tauri:android -- --host 127.0.0.1
   ```

   `--host 127.0.0.1` 会让 Tauri 将调试包的 `devUrl` 改为 `http://127.0.0.1:1420`，同时设置 `TAURI_DEV_HOST`。`vite.config.ts` 会根据该变量监听对应地址，HMR 使用 `1421` 端口。

4. 如已安装旧调试包，需要重新安装或重启对应的 Android 开发构建，使新的 `devUrl` 生效。

5. 调试验证：

   ```bash
   adb reverse --list
   ```

   应看到 `tcp:1420` 和 `tcp:1421`。WebView 当前页面地址应为 `http://127.0.0.1:1420/`，而不是开发机的局域网 IP。
