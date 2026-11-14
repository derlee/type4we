# ADR 0001：Rust 与 Tauri 2

[English](../../en/adr/0001-rust-tauri.md)

## 状态

MVP 采纳。

## 背景

Type4Me 是原生 Swift 菜单栏应用。Windows 移植需要常驻托盘的进程、全局热键、WASAPI 采集、WebSocket 客户端和文字注入。Mac 云端版大约 10–20 MB。Python 边车或 Electron 壳既达不到这个体积，也会给热路径加上不必要的运行时。

设置界面很小：热键、Soniox 密钥和注入方式。不值得上重的前端栈。

## 决定

核心用 Rust。外壳用 Tauri 2，设置页是静态 HTML/CSS/JS，另有一个无边框预览窗。Tauri 使用的 WebView2 在 Windows 11 上已经存在，所以 NSIS 安装包可以是引导程序加上 Rust 二进制，而不是内嵌浏览器。

Win32（底层钩子、`SendInput`、剪贴板、凭据管理器、光标位置）放在 `src-tauri/src/platform`，只在 `cfg(windows)` 下编译。纯逻辑在 `type4we-core`，不需要 Windows 机器就能测试。

## 后果

- 安装包体积和 Mac 云端版同一量级，因为 WebView2 是系统组件。
- CI 必须在 `windows-latest` 上构建。Linux 能跑核心测试，也能对 Windows 目标做类型检查，但不能执行 WASAPI 或键盘钩子。
- 以后的本地识别引擎是 `AsrEngine` 后面的另一个 Rust 类型，不需要换 UI 框架。
