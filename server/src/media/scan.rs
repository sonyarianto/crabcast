//! Media analysis: metadata tag scanning (lofty) and waveform peak
//! computation (symphonia). Runs once at upload time; results are stored in
//! `media_files`.

use std::path::Path;

/// Everything worth persisting about an uploaded file, extracted at upload.
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub duration_seconds: Option<f64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub bitrate: Option<i64>,
    pub replaygain_track_gain: Option<f64>,
    pub replaygain_album_gain: Option<f64>,
    /// (mime, bytes) of the embedded cover art, if any.
    pub cover: Option<(String, Vec<u8>)>,
    /// One amplitude (0..=1) per bucket across the track.
    pub waveform: Vec<f64>,
}

/// Scan a media file for tags, properties, cover art and waveform peaks.
/// Returns `Ok(None)` when the file has no recognized audio stream.
pub fn scan(path: &Path, filename: &str) -> anyhow::Result<Option<ScanResult>> {
    use lofty::file::{AudioFile, TaggedFileExt};
    use lofty::tag::Accessor;

    let tagged = match lofty::read_from_path(path) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };

    let properties = tagged.properties();
    let duration = properties.duration().as_secs_f64();
    let tag = tagged.tags().first();

    let (mut title, mut artist, mut album, mut genre) = (None, None, None, None);
    let (mut replaygain_track, mut replaygain_album) = (None, None);
    let mut cover = None;
    if let Some(tag) = tag {
        title = tag.title().map(|s| s.trim().to_string());
        artist = tag.artist().map(|s| s.trim().to_string());
        album = tag.album().map(|s| s.trim().to_string());
        genre = tag.genre().map(|s| s.trim().to_string());
        replaygain_track =
            parse_replaygain(tag.get_string(lofty::tag::ItemKey::ReplayGainTrackGain));
        replaygain_album =
            parse_replaygain(tag.get_string(lofty::tag::ItemKey::ReplayGainAlbumGain));
        if let Some(picture) = tag.pictures().first() {
            let data = picture.data();
            if !data.is_empty() {
                let mime = picture
                    .mime_type()
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| "image/jpeg".into());
                cover = Some((mime, data.to_vec()));
            }
        }
    }

    // Fall back to the filename stem when tags are empty.
    let stem = Path::new(filename)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| filename.to_string());
    let title = title.filter(|t| !t.is_empty()).unwrap_or(stem);

    let waveform = compute_waveform(path, 256).unwrap_or_default();

    let has_audio = !properties.is_empty() || duration > 0.0 || !waveform.is_empty();
    if !has_audio {
        return Ok(None);
    }

    Ok(Some(ScanResult {
        title,
        artist: artist.unwrap_or_default(),
        album: album.unwrap_or_default(),
        genre: genre.unwrap_or_default(),
        duration_seconds: (duration > 0.0).then_some(duration),
        sample_rate: properties.sample_rate().map(|v| v as i64),
        channels: properties.channels().map(|v| v as i64),
        bitrate: properties
            .audio_bitrate()
            .or_else(|| properties.overall_bitrate())
            .map(|v| v as i64),
        replaygain_track_gain: replaygain_track,
        replaygain_album_gain: replaygain_album,
        cover,
        waveform,
    }))
}

/// lofty reports replaygain as strings like `"-6.5 dB"`; extract the number.
fn parse_replaygain(value: Option<&str>) -> Option<f64> {
    let v = value?.trim();
    let num: String = v
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '.')
        .collect();
    num.parse().ok()
}

/// Decode the file and compute one amplitude peak per bucket (0..=1).
fn compute_waveform(path: &Path, buckets: usize) -> anyhow::Result<Vec<f64>> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;
    let Some(track) = format.default_track().cloned() else {
        return Ok(vec![0.0; buckets]);
    };
    let track_id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    // One (min, max) pair per packet, with its frame range, so bucket
    // boundaries can be resolved after the total frame count is known.
    let mut segments: Vec<(u64, u64, f32, f32)> = Vec::new();
    let mut cursor: u64 = 0;

    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let frames = decoded.frames() as u64;
        if frames == 0 {
            continue;
        }
        let spec = *decoded.spec();
        let mut sbuf = SampleBuffer::<f32>::new(frames, spec);
        sbuf.copy_interleaved_ref(decoded);
        let (mut min, mut max) = (f32::MAX, f32::MIN);
        for &s in sbuf.samples() {
            min = min.min(s);
            max = max.max(s);
        }
        if min <= max {
            segments.push((cursor, cursor + frames, min, max));
            cursor += frames;
        }
    }

    if cursor == 0 {
        return Ok(vec![0.0; buckets]);
    }

    let mut out = vec![0.0f64; buckets];
    for (i, peak) in out.iter_mut().enumerate() {
        let b_start = cursor * i as u64 / buckets as u64;
        let b_end = cursor * (i + 1) as u64 / buckets as u64;
        let mut p = 0.0f32;
        for &(s, e, min, max) in &segments {
            if e > b_start && s < b_end {
                p = p.max(min.abs()).max(max.abs());
            }
        }
        *peak = (p as f64).clamp(0.0, 1.0);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a tiny mono 8 kHz WAV with a 440 Hz sine, ~0.5 s.
    fn write_wav(path: &Path) {
        use std::io::Write;
        let sample_rate = 8000u32;
        let samples: Vec<i16> = (0..sample_rate / 2)
            .map(|i| {
                let t = i as f64 / sample_rate as f64;
                ((t * 440.0 * std::f64::consts::TAU).sin() * 0.5 * i16::MAX as f64) as i16
            })
            .collect();
        let data_len = (samples.len() * 2) as u32;
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(b"RIFF").unwrap();
        f.write_all(&(36 + data_len).to_le_bytes()).unwrap();
        f.write_all(b"WAVE").unwrap();
        f.write_all(b"fmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
        f.write_all(&1u16.to_le_bytes()).unwrap(); // mono
        f.write_all(&sample_rate.to_le_bytes()).unwrap();
        f.write_all(&(sample_rate * 2).to_le_bytes()).unwrap(); // byte rate
        f.write_all(&2u16.to_le_bytes()).unwrap(); // block align
        f.write_all(&16u16.to_le_bytes()).unwrap(); // bits per sample
        f.write_all(b"data").unwrap();
        f.write_all(&data_len.to_le_bytes()).unwrap();
        for s in &samples {
            f.write_all(&s.to_le_bytes()).unwrap();
        }
    }

    #[test]
    fn scan_reads_wav_properties() {
        let dir = std::env::temp_dir().join(format!("crabcast-scan-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tone.wav");
        write_wav(&path);

        let result = scan(&path, "tone.wav").unwrap().expect("should scan");
        assert_eq!(result.title, "tone");
        assert_eq!(result.sample_rate, Some(8000));
        assert_eq!(result.channels, Some(1));
        assert!(result.duration_seconds.unwrap() > 0.4);
        assert!(result.duration_seconds.unwrap() < 0.6);
        // A 440 Hz tone at amplitude 0.5 must produce non-silent peaks.
        assert!(result.waveform.iter().any(|&p| p > 0.1));
        assert_eq!(result.waveform.len(), 256);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn scan_rejects_non_audio() {
        let dir = std::env::temp_dir().join(format!("crabcast-scan-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("not-audio.txt");
        std::fs::write(&path, b"definitely not audio").unwrap();
        assert!(scan(&path, "not-audio.txt").unwrap().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
