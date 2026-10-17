use super::*;

fn wav(channels: u16, rate: u32, bits: u16, data: &[u8]) -> Vec<u8> {
    let mut f = Vec::new();
    f.extend_from_slice(b"RIFF");
    f.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    f.extend_from_slice(b"WAVEfmt ");
    f.extend_from_slice(&16u32.to_le_bytes());
    f.extend_from_slice(&1u16.to_le_bytes());
    f.extend_from_slice(&channels.to_le_bytes());
    f.extend_from_slice(&rate.to_le_bytes());
    let block = channels * bits / 8;
    f.extend_from_slice(&(rate * block as u32).to_le_bytes());
    f.extend_from_slice(&block.to_le_bytes());
    f.extend_from_slice(&bits.to_le_bytes());
    f.extend_from_slice(b"data");
    f.extend_from_slice(&(data.len() as u32).to_le_bytes());
    f.extend_from_slice(data);
    f
}

#[test]
fn decodes_the_shipped_notification_sound() {
    let clip = decode_wav(include_bytes!("../../static/sounds/notification.wav")).unwrap();
    assert_eq!((clip.rate, clip.channels), (48_000, 2));
    let seconds = clip.samples.len() as f64 / 96_000.0;
    assert!((0.5..3.0).contains(&seconds), "{seconds} s");
    assert!(clip.samples.iter().all(|s| (-1.0..=1.0).contains(s)));
    assert!(clip.samples.iter().any(|s| s.abs() > 0.05), "silent");
}

#[test]
fn decodes_16_and_24_bit_samples_with_their_sign() {
    let c16 = decode_wav(&wav(1, 8000, 16, &[0x00, 0x40, 0x00, 0xC0])).unwrap();
    assert_eq!(c16.samples, vec![0.5, -0.5]);
    let c24 = decode_wav(&wav(1, 8000, 24, &[0x00, 0x00, 0x40, 0x00, 0x00, 0xC0])).unwrap();
    assert_eq!(c24.samples, vec![0.5, -0.5]);
}

#[test]
fn refuses_what_it_cannot_play() {
    assert!(decode_wav(b"not a wav").is_err());
    assert!(decode_wav(&wav(1, 8000, 8, &[1, 2])).is_err());
}

#[test]
fn fits_rate_channels_and_volume() {
    let clip = Clip { rate: 24_000, channels: 2, samples: vec![0.5, -0.5, 0.5, -0.5] };
    // Twice the rate: twice the frames; stereo to stereo keeps each side.
    let up = fit(&clip, 48_000, 2, 1.0);
    assert_eq!(up.len(), 8);
    assert!(up.chunks(2).all(|f| f == [0.5, -0.5]), "{up:?}");
    // Stereo to mono averages; volume scales.
    assert!(fit(&clip, 24_000, 1, 0.5).iter().all(|s| s.abs() < 1e-6));
    // Stereo to six channels: the extra ones stay silent.
    let six = fit(&clip, 24_000, 6, 1.0);
    assert_eq!(&six[..6], &[0.5, -0.5, 0.0, 0.0, 0.0, 0.0]);
}
