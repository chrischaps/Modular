//! Opens this machine's real input device through the real output stream.
//!
//! Ignored by default: it needs audio hardware and a microphone. Run with
//!
//! ```text
//! cargo test --release --test live_input -- --ignored --nocapture
//! ```
//!
//! `LIVE_INPUT_SECONDS` sets how long it listens (3 s by default).
//!
//! The patch is Audio Input into the output at volume 0, so nothing is
//! heard and nothing can feed back. It reports what the module's meters
//! heard, and the jitter buffer's latency and dropouts.

use std::time::{Duration, Instant};

use modular_synth::engine::{AudioEngine, AudioProcessor, EngineChannels, EngineCommand, EngineEvent};

#[test]
#[ignore = "needs audio hardware and an input device"]
fn default_input_reaches_the_audio_input_module() {
    let mut engine = AudioEngine::new().expect("an output device");
    let inputs = engine.enumerate_input_devices();
    println!("output: {} at {} Hz", engine.current_device_name(), engine.sample_rate());
    for device in &inputs {
        println!("input {}: {}{}", device.index, device.name, if device.is_default { " (default)" } else { "" });
    }
    let input = inputs.iter().find(|d| d.is_default).or(inputs.first()).expect("an input device");

    let (mut ui, handle) = EngineChannels::with_defaults().split();
    let processor = AudioProcessor::new(engine.sample_rate() as f32, 256, handle);
    engine.start_with_processor(processor).expect("the output stream starts");

    // Audio Input -> output, at volume 0
    ui.send_command(EngineCommand::AddModule { node_id: 1, module_id: "source.audio_input" });
    ui.send_command(EngineCommand::AddModule { node_id: 2, module_id: "output.audio" });
    ui.send_command(EngineCommand::Connect { from_node: 1, from_port: 0, to_node: 2, to_port: 0 });
    ui.send_command(EngineCommand::Connect { from_node: 1, from_port: 1, to_node: 2, to_port: 1 });
    ui.send_command(EngineCommand::SetParameter { node_id: 2, param_index: 0, value: 0.0 });
    ui.send_command(EngineCommand::SetPlaying(true));

    let (feed, monitor) = match engine.open_input(input.index) {
        Ok(opened) => opened,
        Err(e) => panic!("opening {}: {}", input.name, e),
    };
    println!(
        "opened {} ({} channels at {} Hz)",
        engine.input_name().unwrap(),
        engine.input_channels().unwrap(),
        engine.input_sample_rate().unwrap()
    );
    ui.connect_input(feed);

    let mut peak = 0.0f32;
    let mut readings = 0;
    let start = Instant::now();
    let seconds = std::env::var("LIVE_INPUT_SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(3);
    while start.elapsed() < Duration::from_secs(seconds) {
        ui.flush();
        for event in ui.drain_events() {
            if let EngineEvent::MeterLevels { node_id: 1, levels } = event {
                peak = peak.max(levels.peaks[0]).max(levels.peaks[1]);
                readings += 1;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let rate = engine.sample_rate() as f64;
    println!(
        "{readings} meter readings, input peak {:.1} dBFS; buffered {:.1} ms; underruns {:.1} ms, overflows {:.1} ms",
        20.0 * peak.max(1e-9).log10(),
        monitor.buffered_frames() as f64 / rate * 1000.0,
        monitor.underrun_frames() as f64 / rate * 1000.0,
        monitor.overflow_frames() as f64 / rate * 1000.0,
    );
    assert!(!monitor.failed(), "the input stream reported an error");
    assert!(readings > 100, "the Audio Input module ran");
    assert!(monitor.buffered_frames() > 0, "the input was read");
    assert!(peak > 0.0, "the input brought some signal (even a room's hiss)");
}
