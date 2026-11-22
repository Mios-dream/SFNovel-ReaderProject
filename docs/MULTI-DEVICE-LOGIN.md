# 多端登录

两种方式，任选其一。

- **应用内复制 / 粘贴**：应用内点「复制登录凭据」，另一台在登录弹窗里粘贴。不需要电脑，适合手机之间、手机与电脑之间。
- **adb 脚本**：从官方 App 取出登录凭据，登录后可和官方 App 共存。需要电脑。

## adb 脚本

1. 下载 [`SF-Login-Kit.zip`](https://github.com/Mios-dream/SFNovel-ReaderProject/releases/download/tools/SF-Login-Kit.zip)，解压到任意目录。
2. 手机打开「开发者选项 → USB 调试」，连上电脑并在手机上点「允许」；本应用需要先打开过一次。
3. 运行：

   **Windows**：双击 `sf-login.cmd`

   **Linux / macOS**：`./sf-login.sh`

4. 按照提示进行操作。

脚本只是把 adb 命令自动化：清数据、取 `Android/data` 下的日志、解析。有能力的可以照脚本流程手动获取凭证数据，不是必须使用电脑。

> ⚠️ 脚本会清空官方 App 的全部数据（登录状态和已下载内容）。不想清就加 `--skip-clear`。

macOS 没有内置 adb，首次运行按提示加 `--fetch-adb` 现场下载。其他参数用 `--help` 查看。
