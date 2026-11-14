//! Glue between captured audio and an [`AsrWriter`] / [`AsrReader`] pair.
//!
//! The driver is provider-agnostic. Tests feed it a scripted reader so the
//! resample, packetization, and transcript accumulation can run without a
//! network or a microphone.

use super::{
    AsrError, AsrReader, AsrWriter, AudioRequirements, EngineEvent, ServerEvent, TranscriptBuilder,
};
use crate::audio::{f32_to_pcm_s16le, LinearResampler};
use std::time::Duration;
use tokio::sync::{mpsc, watch};

pub type AudioChunk = Vec<f32>;

pub struct DriveSession {
    pub writer: Box<dyn AsrWriter>,
    pub reader: Box<dyn AsrReader>,
    pub audio: mpsc::Receiver<AudioChunk>,
    pub input_sample_rate: u32,
    pub target: AudioRequirements,
    pub stop: watch::Receiver<bool>,
    pub events: mpsc::UnboundedSender<EngineEvent>,
}

pub async fn drive_session(mut drive: DriveSession) {
    let mut resampler =
        match LinearResampler::new(drive.input_sample_rate, drive.target.sample_rate) {
            Ok(resampler) => resampler,
            Err(err) => {
                let _ = drive.events.send(EngineEvent::Failed(err.to_string()));
                return;
            }
        };
    let mut pending = Vec::new();
    let mut builder = TranscriptBuilder::default();
    let frame = drive.target.frame_samples();
    let mut keepalive = tokio::time::interval(Duration::from_secs(15));
    keepalive.tick().await;
    let _ = drive.events.send(EngineEvent::Ready);

    let mut failed: Option<String> = None;
    let mut saw_finished = false;

    loop {
        tokio::select! {
            biased;
            changed = drive.stop.changed() => {
                if changed.is_err() || *drive.stop.borrow() {
                    break;
                }
            }
            chunk = drive.audio.recv() => {
                match chunk {
                    Some(samples) => {
                        pending.extend(resampler.process(&samples));
                        if let Err(err) = send_frames(&mut drive.writer, &mut pending, frame).await {
                            failed = Some(err.to_string());
                            break;
                        }
                    }
                    None => break,
                }
            }
            incoming = drive.reader.recv() => {
                match handle_incoming(incoming, &mut builder, &drive.events) {
                    Incoming::Continue => {}
                    Incoming::Finished => {
                        saw_finished = true;
                        break;
                    }
                    Incoming::Failed(message) => {
                        failed = Some(message);
                        break;
                    }
                }
            }
            _ = keepalive.tick() => {
                if let Err(err) = drive.writer.send_control(super::soniox::keepalive_message()).await {
                    failed = Some(err.to_string());
                    break;
                }
            }
        }
    }

    if failed.is_none() && !saw_finished {
        while let Ok(samples) = drive.audio.try_recv() {
            pending.extend(resampler.process(&samples));
        }
        pending.extend(resampler.flush());
        if let Err(err) = send_frames(&mut drive.writer, &mut pending, 1).await {
            failed = Some(err.to_string());
        } else if let Err(err) = drive.writer.finish().await {
            failed = Some(err.to_string());
        } else {
            let drain = async {
                loop {
                    match handle_incoming(drive.reader.recv().await, &mut builder, &drive.events) {
                        Incoming::Continue => {}
                        Incoming::Finished => return Ok(()),
                        Incoming::Failed(message) => return Err(message),
                    }
                }
            };
            match tokio::time::timeout(Duration::from_secs(5), drain).await {
                Ok(Ok(())) => {}
                Ok(Err(message)) => failed = Some(message),
                Err(_) => {}
            }
        }
    }

    if let Some(message) = failed {
        let _ = drive.events.send(EngineEvent::Failed(message));
        return;
    }
    let remainder = builder.take_partial();
    let _ = drive.events.send(EngineEvent::Finished { remainder });
}

enum Incoming {
    Continue,
    Finished,
    Failed(String),
}

fn handle_incoming(
    incoming: Result<Option<ServerEvent>, AsrError>,
    builder: &mut TranscriptBuilder,
    events: &mpsc::UnboundedSender<EngineEvent>,
) -> Incoming {
    match incoming {
        Ok(Some(ServerEvent::Result { tokens, finished })) => {
            let newly_final = builder.push(&tokens);
            let _ = events.send(EngineEvent::Update {
                newly_final,
                preview: builder.preview(),
            });
            if finished {
                Incoming::Finished
            } else {
                Incoming::Continue
            }
        }
        Ok(Some(ServerEvent::Error { message, .. })) => Incoming::Failed(message),
        Ok(None) => Incoming::Finished,
        Err(err) => Incoming::Failed(err.to_string()),
    }
}

async fn send_frames(
    writer: &mut Box<dyn AsrWriter>,
    pending: &mut Vec<f32>,
    frame: usize,
) -> Result<(), AsrError> {
    let frame = frame.max(1);
    while pending.len() >= frame {
        let piece: Vec<f32> = pending.drain(..frame).collect();
        let bytes = f32_to_pcm_s16le(&piece);
        writer.send_pcm(&bytes).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::{ServerEvent, Token};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    struct ScriptedWriter {
        pcm: Arc<Mutex<Vec<u8>>>,
        controls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl AsrWriter for ScriptedWriter {
        async fn send_pcm(&mut self, pcm_s16le: &[u8]) -> Result<(), AsrError> {
            self.pcm.lock().unwrap().extend_from_slice(pcm_s16le);
            Ok(())
        }
        async fn send_control(&mut self, json: &str) -> Result<(), AsrError> {
            self.controls.lock().unwrap().push(json.to_string());
            Ok(())
        }
        async fn finish(&mut self) -> Result<(), AsrError> {
            self.send_control(r#"{"type":"finalize"}"#).await?;
            self.send_control("").await
        }
    }

    struct ScriptedReader {
        events: Arc<Mutex<Vec<ServerEvent>>>,
    }

    #[async_trait]
    impl AsrReader for ScriptedReader {
        async fn recv(&mut self) -> Result<Option<ServerEvent>, AsrError> {
            loop {
                if let Some(event) = self.events.lock().unwrap().pop() {
                    return Ok(Some(event));
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
    }

    #[tokio::test]
    async fn driver_packetizes_audio_and_accumulates_scripted_tokens() {
        let pcm = Arc::new(Mutex::new(Vec::new()));
        let controls = Arc::new(Mutex::new(Vec::new()));
        let script = Arc::new(Mutex::new(vec![ServerEvent::Result {
            tokens: vec![
                Token {
                    text: "Hi".into(),
                    is_final: true,
                },
                Token {
                    text: " there".into(),
                    is_final: false,
                },
            ],
            finished: false,
        }]));
        // pop() takes from the end, so push the first event last.
        script.lock().unwrap().reverse();

        let (audio_tx, audio_rx) = mpsc::channel(8);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let (stop_tx, stop_rx) = watch::channel(false);

        let drive = tokio::spawn(drive_session(DriveSession {
            writer: Box::new(ScriptedWriter {
                pcm: pcm.clone(),
                controls: controls.clone(),
            }),
            reader: Box::new(ScriptedReader {
                events: script.clone(),
            }),
            audio: audio_rx,
            input_sample_rate: 16_000,
            target: AudioRequirements::PCM_S16LE_16K_MONO,
            stop: stop_rx,
            events: event_tx,
        }));

        assert!(matches!(event_rx.recv().await, Some(EngineEvent::Ready)));
        audio_tx.send(vec![0.25; 1600]).await.unwrap();
        let update = event_rx.recv().await.unwrap();
        match update {
            EngineEvent::Update {
                newly_final,
                preview,
            } => {
                assert_eq!(newly_final, "Hi");
                assert_eq!(preview, "Hi there");
            }
            other => panic!("unexpected {other:?}"),
        }

        script.lock().unwrap().push(ServerEvent::Result {
            tokens: vec![Token {
                text: " there".into(),
                is_final: true,
            }],
            finished: true,
        });

        let mut saw_second = false;
        let mut finished_remainder = None;
        while let Some(event) = event_rx.recv().await {
            match event {
                EngineEvent::Update { newly_final, .. } => {
                    assert_eq!(newly_final, " there");
                    saw_second = true;
                }
                EngineEvent::Finished { remainder } => {
                    finished_remainder = Some(remainder);
                    break;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!(saw_second);
        assert_eq!(finished_remainder.as_deref(), Some(""));
        drive.await.unwrap();
        assert!(!pcm.lock().unwrap().is_empty());
        let _ = stop_tx;
        let _ = controls;
    }

    #[tokio::test]
    async fn stop_flushes_the_tail_and_ends_the_stream() {
        let pcm = Arc::new(Mutex::new(Vec::new()));
        let controls = Arc::new(Mutex::new(Vec::new()));
        let script = Arc::new(Mutex::new(Vec::new()));
        let (audio_tx, audio_rx) = mpsc::channel(4);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let (stop_tx, stop_rx) = watch::channel(false);
        let script_for_task = script.clone();

        let drive = tokio::spawn(async move {
            drive_session(DriveSession {
                writer: Box::new(ScriptedWriter {
                    pcm: pcm.clone(),
                    controls: controls.clone(),
                }),
                reader: Box::new(ScriptedReader {
                    events: script_for_task,
                }),
                audio: audio_rx,
                input_sample_rate: 16_000,
                target: AudioRequirements::PCM_S16LE_16K_MONO,
                stop: stop_rx,
                events: event_tx,
            })
            .await;
            (pcm, controls)
        });

        assert!(matches!(event_rx.recv().await, Some(EngineEvent::Ready)));
        // Shorter than a 100 ms frame, so it is only sent when the session stops.
        audio_tx.send(vec![0.5; 160]).await.unwrap();
        stop_tx.send(true).unwrap();
        script.lock().unwrap().insert(
            0,
            ServerEvent::Result {
                tokens: vec![Token {
                    text: "yo".into(),
                    is_final: false,
                }],
                finished: true,
            },
        );

        let finished = loop {
            match event_rx.recv().await {
                Some(EngineEvent::Finished { remainder }) => break remainder,
                Some(EngineEvent::Failed(message)) => panic!("failed: {message}"),
                Some(_) => {}
                None => panic!("channel closed"),
            }
        };
        assert_eq!(finished, "yo");
        let (pcm, controls) = drive.await.unwrap();
        assert_eq!(pcm.lock().unwrap().len(), 160 * 2);
        let controls = controls.lock().unwrap().clone();
        assert!(controls.iter().any(|item| item == r#"{"type":"finalize"}"#));
        assert!(controls.iter().any(|item| item.is_empty()));
    }
}
