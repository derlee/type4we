# Type4We

[English](README.md)

Type4We 是驻留在系统托盘里的 Windows 语音输入应用。按住热键说话，识别结果会输入到当前光标处。这一版只有**快速模式**：原文转写，不做大模型润色。

它是 [Type4Me](https://github.com/joewongjc/type4me) 的独立 Windows 移植。Type4Me 由 Type4Me 贡献者开发，是 macOS / iOS 菜单栏语音输入应用（按住热键说话，文字出现在光标处，可选大模型润色），采用 MIT 许可证。Type4We 不包含 Type4Me 的源代码。配置里的 `tf_selectedASRProvider`、`tf_asr_soniox`、`tf_language` 以及其他 `tf_*` 分组沿用 Type4Me 的形状，方便以后从 Mac 版 `credentials.json` 导入。许可见 [LICENSE](LICENSE)。

## 这一版能做什么

- 托盘图标：状态、当前模式（快速）、设置、退出。
- 用底层键盘钩子（`WH_KEYBOARD_LL`）做全局热键，支持按住说话和切换。默认是**右 Alt**。Fn 在 Windows 上通常看不到，原因见 [ADR 0004](docs/zh/adr/0004-default-hotkey.md)。
- 用 WASAPI 采集麦克风，混成单声道并重采样到识别器需要的 16 kHz PCM。
- 一家云端流式语音识别：**Soniox**（`stt-rt-v6`）。已确定的 token 会在说话过程中写入光标处。可以在光标附近显示一块实时预览。
- 用 `SendInput` 的 `KEYEVENTF_UNICODE` 注入文字。如果一个事件都没写进去，就改用剪贴板 Ctrl+V，并恢复原来的 Unicode 文本。
- 设置写在 `%APPDATA%\Type4We\config.json`。API 密钥放在 Windows 凭据管理器，不写进 JSON。

这一版不做：大模型模式、历史记录、Ask Anything、Windows 动作、代码签名、自动更新。阶段划分见下方路线图。

## 为什么选 Soniox

Soniox 的公开协议是一个很小的 JSON WebSocket：连到 `wss://stt-rt.soniox.com/transcribe-websocket`，请求头 `Authorization: Bearer <API 密钥>`，先发一条开始消息，再发二进制 `pcm_s16le`。`is_final: false` 的 token 会随语音更新，`is_final: true` 的 token 只出现一次。这足以边说边出字，也可以用官方示例做单元测试。

火山引擎（豆包）流式识别是自定义二进制协议，鉴权方式也更多。它适合以后再接。Type4Me 配置里的非密钥火山字段会被保留，但这一版没有火山客户端。当前模型是 `stt-rt-v6`。`stt-rt-v5` 可用到 2026-12-15，配置里写了它也会照用。文档核对日期 2026-11-14：[WebSocket API](https://docs.soniox.com/api-reference/stt/websocket-api)、[实时转写](https://docs.soniox.com/stt/rt/real-time-transcription)、[模型](https://docs.soniox.com/stt/models)。

## 在 Windows 上构建和运行

需要 Rust stable、[Tauri 依赖](https://v2.tauri.app/start/prerequisites/)（WebView2，Windows 11 已自带）。只有 `cargo tauri dev` 需要 Node.js。

```powershell
git clone https://github.com/derlee/type4we.git
cd type4we
cargo test --workspace
cargo install tauri-cli --version "^2"
cargo tauri build --bundles nsis
```

安装包在 `src-tauri\target\release\bundle\nsis\Type4We_0.1.0_x64-setup.exe`。它按当前用户安装；只有系统没有 WebView2 时才会下载引导程序。开发时运行 `cargo tauri dev`（静态页面在 1420 端口）。

然后：

1. 启动 Type4We。它只出现在托盘里，选「设置」才打开窗口。
2. 在 [console.soniox.com](https://console.soniox.com) 创建密钥，到设置里保存。输入框不会回显密钥，界面只说明有没有保存。
3. 把焦点放在普通（非管理员）文本框，按住右 Alt 说话，松开结束。切换模式则每按一次开始或停止。

详细步骤：[构建](docs/zh/setup.md)、[使用](docs/zh/usage.md)。架构：[概览](docs/zh/architecture.md)。

不含密钥的示例配置是 [`config.example.json`](config.example.json)。

## 已知限制

- **UIPI。** 中等完整性的进程无法把输入送进以管理员身份运行的窗口。`SendInput` 仍可能返回成功。要写入「以管理员身份运行」的程序，需要带 `uiAccess` 的签名二进制，那是第四阶段。见 [ADR 0003](docs/zh/adr/0003-text-injection.md)。
- 很多非美式键盘上**右 Alt 就是 AltGr**。它被设成热键时，钩子会吃掉这个键。请在设置里改成 `Ctrl+Win` 或右 Ctrl。见 [ADR 0004](docs/zh/adr/0004-default-hotkey.md)。
- 剪贴板回退只恢复 **Unicode 文本**。如果当时剪贴板里是文件或图片，这些非文本内容可能丢失。自动模式只在 Unicode 注入报告写入不足时才动剪贴板。
- 这套代码在 Linux 上编写。WASAPI、键盘钩子、`SendInput` 和凭据管理器针对 `x86_64-pc-windows-msvc` 做了类型检查，并由 GitHub Actions 在 `windows-latest` 上编译。那个环境里没有真机，也没有用真实的 Soniox 账号打过线上接口。

## 路线图

- **第二阶段。** 大模型模式、提示词、历史，以及导入 Type4Me 配置（`tf_*` 形状已经留好）。
- **第三阶段。** 用 UI Automation 做 Ask Anything、白名单式的 Windows 动作，以及更多识别引擎（含火山引擎，以及接在同一个 `AsrEngine` trait 后面的本地引擎）。
- **第四阶段。** 代码签名、`uiAccess`（这样才能写入提权窗口）、签名安装包和自动更新。

## 文档

| 中文 | English |
| --- | --- |
| [架构](docs/zh/architecture.md) | [Architecture](docs/en/architecture.md) |
| [构建](docs/zh/setup.md) | [Setup](docs/en/setup.md) |
| [使用](docs/zh/usage.md) | [Usage](docs/en/usage.md) |
| [ADR 0001 Rust + Tauri 2](docs/zh/adr/0001-rust-tauri.md) | [ADR 0001](docs/en/adr/0001-rust-tauri.md) |
| [ADR 0002 云端识别](docs/zh/adr/0002-cloud-asr.md) | [ADR 0002](docs/en/adr/0002-cloud-asr.md) |
| [ADR 0003 文字注入](docs/zh/adr/0003-text-injection.md) | [ADR 0003](docs/en/adr/0003-text-injection.md) |
| [ADR 0004 默认热键](docs/zh/adr/0004-default-hotkey.md) | [ADR 0004](docs/en/adr/0004-default-hotkey.md) |
