//! The notification sound, played by Reach itself on Linux.
//!
//! Everywhere else the page plays it with an `<audio>` element. On Linux that
//! goes through WebKitGTK to GStreamer, and where GStreamer has no audio
//! output (Arch leaves gst-plugins-good optional; the AppImage carries
//! GStreamer's core but none of its plugins), WebKit's web process crashed on
//! the first toast: the window froze, blank, with nothing in the log (issue
//! #87: after restoring a backup, on connecting). Here the sound goes straight
//! to the system's audio (ALSA, PipeWire or PulseAudio through cpal); where
//! there is none, the toast is silent and the log says why.

/// A decoded clip: interleaved samples in -1.0..=1.0.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

/// Decode a PCM WAV file of 16 or 24 bits.
pub fn decode_wav(bytes: &[u8]) -> Result<Clip, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let (mut fmt, mut data) = (None, None);
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &bytes[at + 8..(at + 8 + len).min(bytes.len())];
        match id {
            b"fmt " => fmt = Some(body),
            b"data" => data = Some(body),
            _ => {}
        }
        at += 8 + len + (len & 1);
    }
    let fmt = fmt.filter(|f| f.len() >= 16).ok_or("no fmt chunk")?;
    let data = data.ok_or("no data chunk")?;
    let format = u16::from_le_bytes([fmt[0], fmt[1]]);
    let channels = u16::from_le_bytes([fmt[2], fmt[3]]);
    let rate = u32::from_le_bytes(fmt[4..8].try_into().unwrap());
    let bits = u16::from_le_bytes([fmt[14], fmt[15]]);
    if format != 1 || channels == 0 || rate == 0 {
        return Err(format!("unsupported WAV: format {format}, {channels} channels, {rate} Hz"));
    }
    let samples = match bits {
        16 => data.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0).collect(),
        24 => data
            .chunks_exact(3)
            .map(|s| (i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8) as f32 / 8_388_608.0)
            .collect(),
        _ => return Err(format!("unsupported WAV: {bits} bits")),
    };
    Ok(Clip { rate, channels, samples })
}

/// `clip` as the output device wants it: its rate and channel count, at
/// `volume`. Linear resampling is plenty for a one-second chime.
pub fn fit(clip: &Clip, rate: u32, channels: u16, volume: f32) -> Vec<f32> {
    let src_ch = clip.channels as usize;
    let frames = clip.samples.len() / src_ch;
    if frames == 0 || channels == 0 || rate == 0 {
        return Vec::new();
    }
    let out_frames = (frames as u64 * rate as u64 / clip.rate as u64) as usize;
    let at = |frame: usize, ch: usize| clip.samples[frame.min(frames - 1) * src_ch + ch.min(src_ch - 1)];
    let mut out = Vec::with_capacity(out_frames * channels as usize);
    for i in 0..out_frames {
        let pos = i as f64 * clip.rate as f64 / rate as f64;
        let (f, t) = (pos as usize, (pos.fract()) as f32);
        for ch in 0..channels as usize {
            let s = if channels == 1 && src_ch > 1 {
                (0..src_ch).map(|c| at(f, c) * (1.0 - t) + at(f + 1, c) * t).sum::<f32>() / src_ch as f32
            } else if ch < src_ch || src_ch == 1 {
                at(f, ch) * (1.0 - t) + at(f + 1, ch) * t
            } else {
                0.0
            };
            out.push(s * volume);
        }
    }
    out
}

#[cfg(target_os = "linux")]
mod play {
    use super::{decode_wav, fit, Clip};
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{FromSample, SizedSample};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, LazyLock};
    use std::time::Duration;

    static CLIP: LazyLock<Result<Clip, String>> =
        LazyLock::new(|| decode_wav(include_bytes!("../../static/sounds/notification.wav")));
    /// One chime at a time: a burst of toasts would otherwise stack them up.
    static PLAYING: AtomicBool = AtomicBool::new(false);

    pub fn notification(volume: f32) {
        if PLAYING.swap(true, Ordering::AcqRel) {
            return;
        }
        std::thread::spawn(move || {
            if let Err(e) = play(volume) {
                tracing::info!("Notification sound not played: {e}");
            }
            PLAYING.store(false, Ordering::Release);
        });
    }

    fn play(volume: f32) -> Result<(), String> {
        let clip = CLIP.as_ref().map_err(|e| e.clone())?;
        let device = cpal::default_host().default_output_device().ok_or("no audio output device")?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        let samples = Arc::new(fit(clip, config.sample_rate(), config.channels(), volume));
        let seconds = samples.len() as f64 / (config.sample_rate() as f64 * config.channels() as f64);
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => stream::<f32>(&device, &config.config(), samples),
            cpal::SampleFormat::I16 => stream::<i16>(&device, &config.config(), samples),
            cpal::SampleFormat::I32 => stream::<i32>(&device, &config.config(), samples),
            cpal::SampleFormat::U16 => stream::<u16>(&device, &config.config(), samples),
            other => return Err(format!("output format {other} not handled")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_secs_f64(seconds) + Duration::from_millis(150));
        Ok(())
    }

    fn stream<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        samples: Arc<Vec<f32>>,
    ) -> Result<cpal::Stream, String> {
        let mut next = 0;
        device
            .build_output_stream::<T, _, _>(
                config,
                move |out: &mut [T], _| {
                    for s in out.iter_mut() {
                        *s = T::from_sample(samples.get(next).copied().unwrap_or(0.0));
                        next += 1;
                    }
                },
                |e| tracing::debug!("notification sound: {e}"),
                None,
            )
            .map_err(|e| e.to_string())
    }
}

/// Play the notification sound. Only the Linux page calls this; the others
/// play it themselves.
#[tauri::command]
pub fn play_notification_sound(volume: f32) {
    #[cfg(target_os = "linux")]
    play::notification(volume.clamp(0.0, 1.0));
    #[cfg(not(target_os = "linux"))]
    let _ = volume;
}

#[cfg(test)]
#[path = "sound_tests.rs"]
mod tests;
