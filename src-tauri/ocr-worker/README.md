#桌面OCR运行时

Tauri应用程序将所有SFACG网络请求和Web Cookie保存在Rust中。
此worker仅接收已下载的本地章节图像或GIF。

对于开发，将worker环境与uv同步：

```powershell
uv sync --project src-tauri/ocr-worker
```

默认情况下，worker使用一个OCR worker，以避免耗尽桌面内存
高分辨率GIF。原始图像保留在每本书的
`ocr/目录。识别的UTF-8文本被写入标准输出
无需LLM校正。
当桌面应用程序调用时，分割预览也会保存在
`ocr/segments/chapter-<id>/`：
`_-cutter.png `是空白裁剪框，
`_-strict.png是去除拼音后的框架，
_-line-_.png是个人OCR输入。

worker接受`--segments dir<path>`来启用这些预览。省略
该选项使worker适合不需要中间层的调用者
文件夹。“--拼音顶部裁剪比”控制应用于每个项目的固定顶部裁剪
OCR前的行；它默认为“0.30”，并接受从“0”到“<0.9”的值。

`tauri:dev总是通过以下方式从该目录运行可编辑的ocr_worker.py
其目前的“紫外线”环境，因此工人的变化在当地人之后生效
进程重新启动。发布版本不需要Python。跑
`scripts/build-ocr-worker.ps1在npm运行tauri:build之前；它运行紫外线同步
--组构建，使用PyInstaller和Tauri包创建ocr_worker.exe
该可执行文件位于其资源目录下，以供发布使用。
