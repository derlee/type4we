# ADR 0002：MVP 只用云端识别，但接口可替换

[English](../../en/adr/0002-cloud-asr.md)

## 状态

MVP 采纳。故意不实现本地引擎。

## 背景

Type4Me 可以跑端侧模型（sherpa-onnx 和 Python 边车）。那会带上原生构建、模型文件，在 NVIDIA 机器上还有 CUDA。Windows MVP 要接近 Mac 云端版的体积，并且先把一家识别服务从头到尾跑通。

识别器仍然必须能换。快速模式、热键和注入不应该知道 token 来自哪一家。

## 决定

MVP 只连接云端流式 API。这一家是 Soniox，因为它的公开 WebSocket 是 JSON、用 Bearer 请求头鉴权，并且把中间 token 写清楚了。火山引擎豆包流式接口是二进制协议，控制台鉴权也不止一种，留到后面再接。2026-11-14 核对的文档：

- <https://docs.soniox.com/api-reference/stt/websocket-api>
- <https://docs.soniox.com/stt/rt/real-time-transcription>
- <https://docs.soniox.com/stt/models>

默认模型是 `stt-rt-v6`。配置里如果写了 `stt-rt-v5` 也会接受，该模型计划在 2026-12-15 下线。

扩展点是 `AsrEngine::open`，它返回 `AsrWriter` 和 `AsrReader`。引擎同时报告 `AudioRequirements`。采集路径会重采样到这个格式。以后的本地引擎实现同一组 trait，不用改会话状态机。

不链接 sherpa-onnx、CUDA 或 Python 边车。

## 后果

- 听写需要网络，以及放在凭据管理器里的 Soniox 密钥。
- 协议测试用录好的 JSON 夹具，不用真实账号。
- 加火山或本地模型是新引擎加配置字段，不是重写热键或注入。
