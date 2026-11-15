use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::FromSample;
use tokio::sync::mpsc;
use type4we_core::audio::downmix_to_mono;

pub struct Capture {
    pub sample_rate: u32,
    pub audio: mpsc::Receiver<Vec<f32>>,
    /// Dropping this sender stops the thread that owns the WASAPI stream.
    /// The stream itself is not `Send`, so it cannot live on the async task.
    pub release: std::sync::mpsc::Sender<()>,
}

pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|devices| devices.filter_map(|device| device.name().ok()).collect())
        .unwrap_or_default()
}

pub fn start_capture(device_name: Option<&str>) -> Result<Capture, String> {
    let wanted = device_name.map(str::to_string);
    let (tx, rx) = mpsc::channel(256);
    let (release, keep_alive) = std::sync::mpsc::channel();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("type4we-audio".into())
        .spawn(move || {
            // cpal's WASAPI stream is not Send, so it is created and owned here.
            let started = open_stream(wanted.as_deref(), tx);
            match started {
                Ok((sample_rate, stream)) => {
                    let _ = ready_tx.send(Ok(sample_rate));
                    let _ = keep_alive.recv();
                    drop(stream);
                }
                Err(err) => {
                    let _ = ready_tx.send(Err(err));
                }
            }
        })
        .map_err(|err| err.to_string())?;
    let sample_rate = ready_rx.recv().map_err(|err| err.to_string())??;
    Ok(Capture {
        sample_rate,
        audio: rx,
        release,
    })
}

fn open_stream(
    device_name: Option<&str>,
    tx: mpsc::Sender<Vec<f32>>,
) -> Result<(u32, cpal::Stream), String> {
    let host = cpal::default_host();
    let device = select_device(&host, device_name)?;
    let supported = device
        .default_input_config()
        .map_err(|err| format!("default microphone format: {err}"))?;
    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    let config = supported.config();
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, tx, channels)?,
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, tx, channels)?,
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, tx, channels)?,
        cpal::SampleFormat::I32 => build_stream::<i32>(&device, &config, tx, channels)?,
        other => return Err(format!("unsupported microphone sample format {other}")),
    };
    stream
        .play()
        .map_err(|err| format!("start microphone: {err}"))?;
    Ok((sample_rate, stream))
}

fn select_device(host: &cpal::Host, device_name: Option<&str>) -> Result<cpal::Device, String> {
    if let Some(name) = device_name.map(str::trim).filter(|name| !name.is_empty()) {
        let mut devices = host.input_devices().map_err(|err| err.to_string())?;
        return devices
            .find(|device| device.name().ok().as_deref() == Some(name))
            .ok_or_else(|| format!("microphone `{name}` was not found"));
    }
    host.default_input_device()
        .ok_or_else(|| "no default microphone is available".to_string())
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    tx: mpsc::Sender<Vec<f32>>,
    channels: usize,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _info: &cpal::InputCallbackInfo| {
                let samples: Vec<f32> = data
                    .iter()
                    .copied()
                    .map(cpal::Sample::to_sample::<f32>)
                    .collect();
                let mono = downmix_to_mono(&samples, channels.max(1));
                if !mono.is_empty() {
                    let _ = tx.try_send(mono);
                }
            },
            |err| tracing::error!(error = %err, "microphone stream error"),
            None,
        )
        .map_err(|err| format!("open microphone: {err}"))
}
