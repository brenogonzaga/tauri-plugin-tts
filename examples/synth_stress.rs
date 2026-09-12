//! Hammers the buffer-producing synthesizer the way a sentence pool does:
//! `workers` jobs in flight at once, each restarted the moment it ends, for
//! `rounds` completions. Run on macOS:
//! `cargo run --example synth_stress -- [rounds] [workers] [language]`.
//!
//! Exists to reproduce (and then guard against) the framework-side
//! use-after-free that a per-job `AVSpeechSynthesizer` released right after
//! its last buffer can trigger on the main queue.

#[cfg(target_os = "macos")]
fn main() {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    use tauri_plugin_tts::{SynthesisRequest, SynthesizerEvent};

    let mut args = std::env::args().skip(1);
    let rounds: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(60);
    let workers: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(3);
    let language = args.next().unwrap_or_else(|| "de-DE".to_string());
    let sentences = ["Eins.", "Zwei drei.", "Vier fünf sechs.", "Sieben."];

    let synth = tauri_plugin_tts::apple_synthesizer();
    let (tx, rx) = mpsc::channel::<(usize, SynthesizerEvent)>();
    let mut jobs = Vec::new();
    let mut started = 0usize;
    let start = |slot: usize,
                 jobs: &mut Vec<Box<dyn tauri_plugin_tts::SynthesisJob>>,
                 started: &mut usize| {
        let tx = tx.clone();
        let text = sentences[*started % sentences.len()];
        *started += 1;
        let job = synth
            .synthesize(
                SynthesisRequest {
                    language: Some(language.clone()),
                    ..SynthesisRequest::new(text)
                },
                Box::new(move |event| {
                    let _ = tx.send((slot, event));
                }),
            )
            .expect("job starts");
        jobs.push(job);
    };
    for slot in 0..workers {
        start(slot, &mut jobs, &mut started);
    }

    let t0 = Instant::now();
    let mut ended = 0usize;
    let mut samples = 0usize;
    let deadline = Instant::now() + Duration::from_secs(120);
    while ended < rounds && Instant::now() < deadline {
        tauri_plugin_tts::pump_main_run_loop(0.005);
        while let Ok((slot, event)) = rx.try_recv() {
            match event {
                SynthesizerEvent::Audio { samples: s, .. } => samples += s.len(),
                SynthesizerEvent::Error(e) => panic!("slot {slot}: synthesis error: {e}"),
                SynthesizerEvent::Ended => {
                    ended += 1;
                    if started < rounds + workers {
                        start(slot, &mut jobs, &mut started);
                    }
                }
            }
        }
    }
    println!(
        "{ended}/{rounds} jobs ended in {:?} ({} samples, {} jobs started)",
        t0.elapsed(),
        samples,
        started
    );
    assert!(ended >= rounds, "only {ended} of {rounds} jobs ended");
    // Let any late framework callbacks land while we are still alive.
    tauri_plugin_tts::pump_main_run_loop(1.0);
    println!("no crash");
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("synth_stress only runs on macOS");
}
