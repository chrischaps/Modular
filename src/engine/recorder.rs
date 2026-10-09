//! Recording what you hear to a WAV file.
//!
//! The audio thread holds a [`RecordTap`]: after each callback it copies the
//! finished output buffer, interleaved and after the output limiter, into a
//! ring of about two seconds. It never waits; if the ring is full the frames
//! that don't fit are counted as dropped. A [`Recording`] owns the other end
//! on a writer thread, which drains the ring into a 32-bit float WAV at the
//! device's rate and channel count.
//!
//! The tap reaches the audio thread inside an [`AudioMessage`], and comes
//! back to the UI thread the same way a replaced plan does, so the ring is
//! never freed on the audio thread. Once the UI drops the returned tap, the
//! writer sees the ring abandoned, writes what's left and finalizes the file.
//!
//! [`AudioMessage`]: super::commands::AudioMessage

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{fence, AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use web_time::Instant;

use rtrb::{Consumer, Producer, RingBuffer};

/// How much audio the ring holds while the writer catches up.
pub const RING_SECONDS: f32 = 2.0;

/// How often the writer wakes to drain the ring.
const WRITER_POLL: Duration = Duration::from_millis(10);

/// How often the writer rewrites the WAV header, so a crash leaves a file
/// that plays up to the last second or so.
const HEADER_REFRESH: Duration = Duration::from_secs(1);

/// Counters the audio thread keeps for the UI.
#[derive(Debug, Default)]
struct RecordStats {
    /// Frames copied into the ring.
    frames: AtomicU64,
    /// Frames that didn't fit in the ring.
    dropped: AtomicU64,
}

/// The audio thread's end of a recording.
pub struct RecordTap {
    producer: Producer<f32>,
    channels: usize,
    stats: Arc<RecordStats>,
}

impl RecordTap {
    /// Copies one callback's interleaved output into the ring: as many whole
    /// frames as fit, counting the rest as dropped.
    ///
    /// REAL-TIME SAFE: no allocation, locking or waiting.
    pub fn write(&mut self, output: &[f32], channels: usize) {
        if channels == 0 {
            return;
        }
        let frames = output.len() / channels;
        // Another device's buffers don't belong in this file
        let fit = if channels == self.channels { (self.producer.slots() / channels).min(frames) } else { 0 };
        if fit > 0 {
            if let Ok(chunk) = self.producer.write_chunk_uninit(fit * channels) {
                chunk.fill_from_iter(output[..fit * channels].iter().copied());
            }
        }
        self.stats.frames.fetch_add(fit as u64, Ordering::Relaxed);
        if fit < frames {
            self.stats.dropped.fetch_add((frames - fit) as u64, Ordering::Relaxed);
        }
    }
}

/// What a finished recording wrote.
#[derive(Debug, Clone)]
pub struct RecordingSummary {
    pub path: PathBuf,
    /// Frames in the file.
    pub frames: u64,
    pub sample_rate: u32,
    pub channels: u16,
    /// Frames lost because the writer fell more than the ring behind.
    pub dropped_frames: u64,
    /// Why the writer stopped early, if it did (a full disk, say). The file
    /// holds everything written before that.
    pub error: Option<String>,
}

impl RecordingSummary {
    /// The length of the file.
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.frames as f64 / self.sample_rate.max(1) as f64)
    }
}

/// The UI thread's end of a recording: the file, and the thread writing it.
pub struct Recording {
    path: PathBuf,
    sample_rate: u32,
    channels: u16,
    stats: Arc<RecordStats>,
    /// Tells the writer to finish without waiting for the tap to come back.
    force_finish: Arc<AtomicBool>,
    writer: Option<JoinHandle<(u64, Option<String>)>>,
    /// When the tap was let go, if it has been.
    stopping_since: Option<Instant>,
}

impl Recording {
    /// Creates the WAV file and starts its writer. Send the returned tap to
    /// the audio thread to start recording.
    pub fn start(path: &Path, sample_rate: u32, channels: u16) -> Result<(Self, RecordTap), String> {
        if channels == 0 || sample_rate == 0 {
            return Err("the output has no channels".to_string());
        }
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let wav = hound::WavWriter::create(path, spec).map_err(|e| format!("can't create {}: {}", path.display(), e))?;

        let capacity = (sample_rate as f32 * RING_SECONDS) as usize * channels as usize;
        let (producer, consumer) = RingBuffer::new(capacity);
        let stats = Arc::new(RecordStats::default());
        let force_finish = Arc::new(AtomicBool::new(false));

        let writer = {
            let force_finish = Arc::clone(&force_finish);
            std::thread::Builder::new()
                .name("recorder".into())
                .spawn(move || write_until_done(consumer, wav, &force_finish))
                .map_err(|e| format!("can't start the recorder: {}", e))?
        };

        let tap = RecordTap { producer, channels: channels as usize, stats: Arc::clone(&stats) };
        let recording = Self {
            path: path.to_path_buf(),
            sample_rate,
            channels,
            stats,
            force_finish,
            writer: Some(writer),
            stopping_since: None,
        };
        Ok((recording, tap))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How much audio has been recorded, by the device's clock.
    pub fn elapsed(&self) -> Duration {
        let frames = self.stats.frames.load(Ordering::Relaxed);
        Duration::from_secs_f64(frames as f64 / self.sample_rate as f64)
    }

    pub fn dropped_frames(&self) -> u64 {
        self.stats.dropped.load(Ordering::Relaxed)
    }

    /// Notes that the tap has been asked back from the audio thread. The
    /// file finishes once it's returned and dropped.
    pub fn mark_stopping(&mut self) {
        self.stopping_since.get_or_insert_with(Instant::now);
    }

    /// Whether the tap has been asked back.
    pub fn is_stopping(&self) -> bool {
        self.stopping_since.is_some()
    }

    /// Whether the writer has finished the file, so [`finish`](Self::finish)
    /// won't wait. Past `patience` after stopping, the writer is told to
    /// finish without the tap (the audio thread isn't running to return it).
    pub fn poll_finished(&self, patience: Duration) -> bool {
        if self.stopping_since.is_some_and(|since| since.elapsed() > patience) {
            self.force_finish.store(true, Ordering::Relaxed);
        }
        self.writer.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Waits for the writer to finalize the file, giving the tap `patience`
    /// to come back first.
    pub fn finish(mut self, patience: Duration) -> RecordingSummary {
        self.mark_stopping();
        while !self.poll_finished(patience) {
            std::thread::sleep(Duration::from_millis(2));
        }
        let (frames, error) = match self.writer.take().map(JoinHandle::join) {
            Some(Ok(result)) => result,
            Some(Err(_)) => (0, Some("the recorder crashed".to_string())),
            None => (0, None),
        };
        RecordingSummary {
            path: self.path.clone(),
            frames,
            sample_rate: self.sample_rate,
            channels: self.channels,
            dropped_frames: self.dropped_frames(),
            error,
        }
    }
}

impl Drop for Recording {
    /// A recording let go without [`finish`](Recording::finish) still ends
    /// with a playable file.
    fn drop(&mut self) {
        if let Some(writer) = self.writer.take() {
            self.force_finish.store(true, Ordering::Relaxed);
            let _ = writer.join();
        }
    }
}

/// The writer thread: drains the ring into the file until the tap is let go
/// (or it's told to stop), then finalizes. Returns the frames written, and
/// the error that stopped it early, if any.
fn write_until_done(
    mut consumer: Consumer<f32>,
    mut wav: hound::WavWriter<BufWriter<File>>,
    force_finish: &AtomicBool,
) -> (u64, Option<String>) {
    let channels = wav.spec().channels as u64;
    let mut last_header = Instant::now();
    let mut error = None;

    loop {
        // Checked before draining: once the tap is gone, everything it wrote
        // is already in the ring. The fence pairs with the producer's drop
        let done = consumer.is_abandoned() || force_finish.load(Ordering::Relaxed);
        fence(Ordering::Acquire);

        if let Err(e) = drain(&mut consumer, &mut wav) {
            error = Some(e.to_string());
            break;
        }
        if done {
            break;
        }
        if last_header.elapsed() >= HEADER_REFRESH {
            last_header = Instant::now();
            if let Err(e) = wav.flush() {
                error = Some(e.to_string());
                break;
            }
        }
        std::thread::sleep(WRITER_POLL);
    }

    let frames = wav.len() as u64 / channels;
    if let Err(e) = wav.finalize() {
        error.get_or_insert(e.to_string());
    }
    (frames, error)
}

/// Writes everything waiting in the ring to the file.
fn drain(consumer: &mut Consumer<f32>, wav: &mut hound::WavWriter<BufWriter<File>>) -> hound::Result<()> {
    let available = consumer.slots();
    if available == 0 {
        return Ok(());
    }
    let chunk = consumer.read_chunk(available).expect("slots() were available");
    let (first, second) = chunk.as_slices();
    for &sample in first.iter().chain(second) {
        wav.write_sample(sample)?;
    }
    chunk.commit_all();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_wav(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("modular-recorder-tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn read_back(path: &Path) -> (hound::WavSpec, Vec<f32>) {
        let mut reader = hound::WavReader::open(path).unwrap();
        let spec = reader.spec();
        (spec, reader.samples::<f32>().map(Result::unwrap).collect())
    }

    #[test]
    fn writes_what_the_tap_hears() {
        let path = temp_wav("tap.wav");
        let (recording, mut tap) = Recording::start(&path, 48000, 2).unwrap();
        let mut heard = Vec::new();
        for block in 0..50 {
            let buffer: Vec<f32> = (0..256 * 2).map(|i| (block * 1000 + i) as f32 * 1e-6).collect();
            tap.write(&buffer, 2);
            heard.extend_from_slice(&buffer);
        }
        drop(tap);
        let summary = recording.finish(Duration::from_secs(5));

        assert!(summary.error.is_none());
        assert_eq!(summary.dropped_frames, 0);
        assert_eq!(summary.frames, 50 * 256);
        let (spec, samples) = read_back(&path);
        assert_eq!((spec.channels, spec.sample_rate, spec.bits_per_sample), (2, 48000, 32));
        assert_eq!(samples, heard);
    }

    #[test]
    fn a_full_ring_drops_whole_frames_and_counts_them() {
        let path = temp_wav("full.wav");
        // A tiny rate makes a tiny ring: 2 s at 100 Hz is 200 frames
        let (recording, mut tap) = Recording::start(&path, 100, 2).unwrap();
        // Hold the writer off by filling faster than it can poll
        let buffer = vec![0.25_f32; 300 * 2];
        tap.write(&buffer, 2);
        assert_eq!(recording.dropped_frames(), 100);
        assert_eq!(recording.elapsed(), Duration::from_secs(2));

        // A buffer from a device with another channel count is dropped whole
        tap.write(&[0.0; 6], 3);
        assert_eq!(recording.dropped_frames(), 102);
        drop(tap);
        let summary = recording.finish(Duration::from_secs(5));
        assert_eq!(summary.frames, 200);
        assert_eq!(summary.dropped_frames, 102);
    }

    #[test]
    fn finishing_without_the_tap_still_finalizes() {
        let path = temp_wav("forced.wav");
        let (recording, mut tap) = Recording::start(&path, 48000, 1).unwrap();
        tap.write(&[0.5; 480], 1);
        // The audio thread never hands the tap back
        let summary = recording.finish(Duration::from_millis(20));
        assert_eq!(summary.frames, 480);
        let (_, samples) = read_back(&path);
        assert_eq!(samples.len(), 480);
        drop(tap);
    }
}
