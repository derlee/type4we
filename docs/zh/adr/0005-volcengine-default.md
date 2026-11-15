# ADR 0005：默认使用火山引擎

[English](../../en/adr/0005-volcengine-default.md)

## 状态

采纳。取代 [ADR 0002](0002-cloud-asr.md) 里「只接 Soniox」的部分。那篇里的 `AsrEngine` trait 不变。

## 背景

MVP 先接了 Soniox，因为它的公开接口是 JSON WebSocket，协议更容易先跑通。仓库主人要求默认云端引擎改为火山引擎的豆包流式语音识别，Soniox 保留为可选项。

## 决定

默认的 `tf_selectedASRProvider` 是 `volcano`，和 Type4Me 的原始值相同。设置里火山引擎排在第一位。

客户端按官方大模型二进制协议实现，2026-11-15 核对：

- <https://www.volcengine.com/docs/6561/1354869>
- <https://docs.volcengine.com/docs/doubao-voice/large-model-streaming-automatic-speech-recognition-api-1>

实时听写使用 `wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async`。文档建议用这个双向流式优化版，而不是每个包都回的 `/bigmodel`，也不是要等 15 秒或最后一包才出结果的 `/bigmodel_nostream`。音频是 16 kHz 单声道 PCM s16le，按 200 ms 一包 gzip 发送。同一页写明双向流式在 200 ms 时效果最好。

每帧都是大端：4 字节头、可选的 int32 序号、uint32 负载长度、负载。完整客户端请求是 JSON、gzip、不带序号（`11 10 11 00`）。音频帧带正序号（`11 21 01 00`）。最后一包打上结束标志和负序号（`11 23 01 00`）。服务端错误是消息类型 `0xF`，带大端错误码和 UTF-8 说明。标点和 ITN 打开。不传 `language`，以便默认模型接受中英混合。打开 `show_utterances` 和 `enable_nonstream`，这样说完的句子会带 `definite`。

握手头在旧版控制台是 `X-Api-App-Key` 和 `X-Api-Access-Key`，新版控制台是 `X-Api-Key`，再加上 `X-Api-Resource-Id`、`X-Api-Request-Id`、`X-Api-Connect-Id` 和 `X-Api-Sequence: -1`。默认资源 ID 是 `volc.seedasr.sauc.duration`（2.0，按时长计费）。也可以选 2.0 并发版，以及 1.0 的 `volc.bigasr.sauc.duration` / `volc.bigasr.sauc.concurrent`。

`appKey`、`accessKey` 和 `apiKey` 存在凭据管理器的 `tf_asr_volcano` 下，字段名和 Type4Me 一致。`authMode` 和 `resourceId` 留在 JSON 的 `tf_asr_volcano` 里，Mac 导入也落在这里。每次会话新建一条连接，序号从 1 开始。

Soniox 仍然编译在内。在设置里改选它，不需要动热键、采集或注入。

## 后果

- 默认路径需要已开通流式识别的火山引擎账号。CI 里没有真实请求。
- 帧的编码和解码、gzip 负载、序号和最后一包标志、分句最终结果、错误码都有不联网的单元测试。设置了 `VOLC_APP_KEY` 和 `VOLC_ACCESS_KEY` 时，有一条被忽略的联调测试可以手动跑。
- ADR 0003 里提权窗口的注入限制没有变化。
