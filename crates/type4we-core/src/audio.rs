//! Sample conversion and streaming resample. Capture itself is WASAPI in the
//! app crate; these functions are what the capture callback and the session
//! driver share.

/// Average interleaved channels into mono.
pub fn downmix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

pub fn pcm_s16le_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| i16::from_le_bytes(*chunk) as f32 / i16::MAX as f32)
        .collect()
}

pub fn f32_to_pcm_s16le(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let value = (clamped * i16::MAX as f32).round() as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// Streaming linear resampler. `ratio` is input rate / output rate.
///
/// One sample of lookahead is retained so chunk boundaries do not click.
#[derive(Debug, Clone)]
pub struct LinearResampler {
    step: f64,
    /// Index of the next output sample, relative to the start of `held`.
    next: f64,
    held: Vec<f32>,
}

impl LinearResampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Result<Self, AudioError> {
        if input_rate == 0 || output_rate == 0 {
            return Err(AudioError::InvalidSampleRate);
        }
        Ok(Self {
            step: input_rate as f64 / output_rate as f64,
            next: 0.0,
            held: Vec::new(),
        })
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if input.is_empty() {
            return Vec::new();
        }
        self.held.extend_from_slice(input);
        let mut out = Vec::new();
        if (self.step - 1.0).abs() < f64::EPSILON {
            let count = self.held.len();
            out.extend_from_slice(&self.held);
            self.held.clear();
            self.next = 0.0;
            let _ = count;
            return out;
        }
        while self.next + 1.0 < self.held.len() as f64 {
            let index = self.next.floor() as usize;
            let frac = (self.next - index as f64) as f32;
            let sample = self.held[index] * (1.0 - frac) + self.held[index + 1] * frac;
            out.push(sample);
            self.next += self.step;
        }
        let drop_count = self.next.floor() as usize;
        if drop_count > 0 && drop_count <= self.held.len() {
            self.held.drain(..drop_count);
            self.next -= drop_count as f64;
        }
        out
    }

    /// Emit any sample still represented by the tail, then drop the buffer.
    pub fn flush(&mut self) -> Vec<f32> {
        if self.held.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        if (self.step - 1.0).abs() < f64::EPSILON {
            out.append(&mut self.held);
            self.next = 0.0;
            return out;
        }
        while self.next < self.held.len() as f64 {
            let index = self.next.floor() as usize;
            let frac = (self.next - index as f64) as f32;
            let right = if index + 1 < self.held.len() {
                self.held[index + 1]
            } else {
                self.held[index]
            };
            let sample = self.held[index] * (1.0 - frac) + right * frac;
            out.push(sample);
            self.next += self.step;
        }
        self.held.clear();
        self.next = 0.0;
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AudioError {
    #[error("sample rate must be non-zero")]
    InvalidSampleRate,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_stereo() {
        let mono = downmix_to_mono(&[0.0, 1.0, 0.5, 0.5], 2);
        assert_eq!(mono, vec![0.5, 0.5]);
    }

    #[test]
    fn mono_passthrough() {
        let samples = vec![0.25, -0.25];
        assert_eq!(downmix_to_mono(&samples, 1), samples);
    }

    #[test]
    fn i16_roundtrip_preserves_silence_and_full_scale() {
        let encoded = f32_to_pcm_s16le(&[0.0, 1.0, -1.0]);
        let decoded = pcm_s16le_to_f32(&encoded);
        assert!(decoded[0].abs() < 1e-4);
        assert!((decoded[1] - 1.0).abs() < 1e-3);
        assert!((decoded[2] + 1.0).abs() < 1e-3);
    }

    #[test]
    fn resample_48k_to_16k_shrinks_by_about_three() {
        let mut resampler = LinearResampler::new(48_000, 16_000).unwrap();
        let input: Vec<f32> = (0..4800).map(|i| (i as f32 / 100.0).sin()).collect();
        let mut output = resampler.process(&input);
        output.extend(resampler.flush());
        assert!(
            (1400..1800).contains(&output.len()),
            "expected about 1600 samples, got {}",
            output.len()
        );
    }

    #[test]
    fn equal_rates_copy_samples() {
        let mut resampler = LinearResampler::new(16_000, 16_000).unwrap();
        let input = vec![0.1, 0.2, 0.3, 0.4];
        assert_eq!(resampler.process(&input), input);
    }

    #[test]
    fn constant_signal_survives_resample() {
        let mut resampler = LinearResampler::new(48_000, 16_000).unwrap();
        let input = vec![0.5; 480];
        let output = resampler.process(&input);
        assert!(!output.is_empty());
        for sample in output {
            assert!((sample - 0.5).abs() < 1e-4);
        }
    }

    #[test]
    fn zero_sample_rate_is_rejected() {
        assert!(LinearResampler::new(0, 16_000).is_err());
    }
}
