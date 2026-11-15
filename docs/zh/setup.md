# 构建

[English](../en/setup.md)

## 环境

- 64 位 Windows 10 或 11。
- [Rust](https://rustup.rs/) stable（1.90 或更新；Tauri 2.12 要求 1.90）。
- [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)。现在的 Windows 11 已经自带。NSIS 安装包在缺失时才下载引导程序，所以安装包本身不大。
- 只有 `cargo tauri dev` 需要 Node.js，它用来提供静态的 `ui/` 目录。`cargo tauri build` 直接拷贝 `ui/`，不需要 Node。

Windows 上的 Rust 需要带 MSVC 工具链的 Visual Studio Build Tools。`rustup` 会指向它们。

## 构建

```powershell
cargo test --workspace
cargo install tauri-cli --version "^2"
cargo tauri build --bundles nsis
```

产物：

- `target\release\type4we.exe`
- `target\release\bundle\nsis\Type4We_0.1.0_x64-setup.exe`

Cargo 把产物放在工作区的 `target` 目录，不在 `src-tauri` 下面。

安装包按当前用户安装，不会要求管理员。因此进程是中等完整性，不能向提权窗口打字。见 [ADR 0003](adr/0003-text-injection.md)。

开发：

```powershell
cargo tauri dev
```

这会运行 `node scripts/dev-server.cjs`，在 `http://127.0.0.1:1420` 提供 `ui/`。

## 持续集成

`.github/workflows/ci.yml` 在 `windows-latest` 上运行。它执行 `cargo test --workspace --locked`，用 `tauri-apps/tauri-action` 打 NSIS 包，并把安装包和 `type4we.exe` 上传为 `type4we-windows` 产物。

核心库测试不需要 Windows。它们覆盖会话状态机、配置读写（包括剥掉 API 密钥）、热键和弦、重采样，以及从公开文档摘下来的 Soniox 消息夹具。

## 配置文件

应用读取 `%APPDATA%\Type4We\config.json`。文件不存在时，第一次保存会按默认值创建。[`config.example.json`](../../config.example.json) 是同一形状、不含密钥的例子。

API 密钥不属于这个文件。如果拷进来的 Type4Me `credentials.json` 在 `tf_asr_*` 或 `tf_llm_*` 对象里仍有 `apiKey` 或 `accessKey`，第一次加载会把这些字符串放进凭据管理器，并写回不含密钥的 JSON。凭据目标名是 `com.type4we.grouped:tf_asr_soniox:apiKey`。

日志追加到 `%APPDATA%\Type4We\logs\type4we.log`。

## 火山引擎凭据

1. 登录 <https://console.volcengine.com/>。
2. 打开豆包语音，开通流式语音识别。现在更建议用 2.0；1.0 仍可用。
3. 创建应用。旧版控制台给出 App ID 和 Access Token。新版控制台给出 API Key。Type4We 两种都收：鉴权保持「App ID + Access Token」，或改成「API Key」。
4. 选择资源 ID。默认 `volc.seedasr.sauc.duration` 是 2.0 按时长计费。`volc.seedasr.sauc.concurrent` 是并发版。`volc.bigasr.sauc.duration` 和 `volc.bigasr.sauc.concurrent` 是 1.0 的对应项。这里写错时，服务会报未授权，而不是令牌无效。
5. 在 Type4We 设置里保存。Token 或 API Key 进入 Windows 凭据管理器。`resourceId` 和 `authMode` 写到 `%APPDATA%\Type4We\config.json` 的 `tf_asr_volcano`。

若改用 Soniox，在同一页切换服务，并到 <https://console.soniox.com> 保存密钥。
