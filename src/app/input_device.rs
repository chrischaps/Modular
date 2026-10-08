//! Small decisions about the audio input device that the toolbar and status
//! bar share.

use std::time::Duration;

/// Whether an output device's name says it's worn on the head, so a live
/// input can't feed back through it. Anything else is treated as speakers.
pub fn looks_like_headphones(output_name: &str) -> bool {
    let name = output_name.to_lowercase();
    ["headphone", "headset", "earphone", "earbud", "buds", "airpods", "in-ear", "hands-free"]
        .iter()
        .any(|word| name.contains(word))
}

/// The one-line warning shown the first time an input is opened while the
/// output plays through speakers.
pub fn feedback_warning(output_name: &str) -> String {
    format!(
        "Input is live and the output is {}: use headphones or keep the volume low, so the input doesn't hear the speakers and howl",
        output_name
    )
}

/// A sample rate as people say it: "48 kHz", "44.1 kHz".
pub fn khz(rate: u32) -> String {
    format!("{} kHz", rate as f32 / 1000.0)
}

/// How long the jitter buffer's latency is, for the status bar.
pub fn latency(frames: usize, sample_rate: u32) -> Duration {
    Duration::from_secs_f64(frames as f64 / sample_rate.max(1) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headphones_are_recognised_by_name() {
        for name in [
            "Headphones (Realtek(R) Audio)",
            "Headset Earphone (Jabra Evolve 65)",
            "AirPods Pro",
            "Galaxy Buds2 Pro",
            "Headset (WH-1000XM4 Hands-Free AG Audio)",
        ] {
            assert!(looks_like_headphones(name), "{name}");
        }
        for name in ["Speakers (Realtek(R) Audio)", "LG ULTRAGEAR (NVIDIA High Definition Audio)", "Focusrite USB Audio"] {
            assert!(!looks_like_headphones(name), "{name}");
        }
    }

    #[test]
    fn test_rates_read_as_said() {
        assert_eq!(khz(48000), "48 kHz");
        assert_eq!(khz(44100), "44.1 kHz");
    }

    #[test]
    fn test_latency_in_time() {
        assert_eq!(latency(480, 48000), Duration::from_millis(10));
    }
}
