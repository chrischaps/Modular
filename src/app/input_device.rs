//! Small decisions about the audio input device that the toolbar and status
//! bar share.

use std::time::Duration;

use crate::engine::RoundTrip;

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

/// A delay in whole milliseconds: "21 ms".
pub fn ms(delay: Duration) -> String {
    format!("{:.0} ms", delay.as_secs_f64() * 1000.0)
}

/// The status bar's label for an open input: the round trip, marked as a
/// floor ("In 22+ ms") when a device doesn't report its own delay.
pub fn round_trip_label(trip: &RoundTrip) -> String {
    let total = trip.total().as_secs_f64() * 1000.0;
    if trip.complete() {
        format!("In {total:.0} ms")
    } else {
        format!("In {total:.0}+ ms")
    }
}

/// The label's hover: the round trip and what it's made of.
pub fn round_trip_details(trip: &RoundTrip) -> String {
    let device = |delay: Option<Duration>| delay.map_or_else(|| "not reported".to_string(), ms);
    let input = match trip.input_device {
        Some(delay) if trip.input_estimated => format!("~{}", ms(delay)),
        delay => device(delay),
    };
    let parts = format!(
        "Input device {} · Buffer {} · Output device {} · Limiter {}",
        input,
        ms(trip.buffer),
        device(trip.output_device),
        ms(trip.limiter)
    );
    let total = ms(trip.total());
    let summary = match (trip.input_device, trip.output_device) {
        (Some(_), Some(_)) => format!("Round trip {total}, from the input jack to the speakers"),
        (None, Some(_)) => format!("Round trip at least {total}: the input device doesn't report its own delay"),
        (Some(_), None) => format!("Round trip at least {total}: the output device doesn't report its own delay"),
        (None, None) => format!("Round trip at least {total}: the devices don't report their own delay"),
    };
    if trip.input_estimated {
        format!("{summary}\n{parts}\nThe input device doesn't time its own delay, so it's counted as one packet: the least it can be")
    } else {
        format!("{summary}\n{parts}")
    }
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
    fn test_round_trip_reads_as_its_parts() {
        let trip = RoundTrip::new(Some(Duration::from_millis(12)), 480, 1008, Some(Duration::from_millis(14)), 48000);
        assert_eq!(round_trip_label(&trip), "In 48 ms");
        let details = round_trip_details(&trip);
        assert!(details.starts_with("Round trip 48 ms"), "{details}");
        assert!(details.ends_with("Input device 12 ms · Buffer 21 ms · Output device 14 ms · Limiter 1 ms"), "{details}");
    }

    #[test]
    fn test_round_trip_without_timestamps_says_it_is_a_floor() {
        let trip = RoundTrip::new(None, 0, 1008, None, 48000);
        assert_eq!(round_trip_label(&trip), "In 22+ ms");
        let details = round_trip_details(&trip);
        assert!(details.contains("the devices don't report"), "{details}");
        assert!(details.contains("Input device not reported"), "{details}");

        let trip = RoundTrip::new(Some(Duration::from_millis(12)), 480, 1008, None, 48000);
        assert!(round_trip_details(&trip).contains("the output device doesn't"));
    }

    #[test]
    fn test_estimated_input_is_marked() {
        // Windows: the input's delay is one 10 ms packet
        let trip = RoundTrip::new(None, 480, 1008, Some(Duration::from_millis(10)), 48000);
        assert_eq!(round_trip_label(&trip), "In 42 ms");
        let details = round_trip_details(&trip);
        assert!(details.contains("Input device ~10 ms · Buffer 21 ms"), "{details}");
        assert!(details.contains("counted as one packet"), "{details}");
    }
}
