//! Phrases rendered by the subtractive voice (`subtractive-mono/1`, DESIGN.md § 5.2), for the
//! note estimator's tests. A dev-dependency on `patchback-dsp`, which does not depend on this
//! crate.

#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

use patchback_core::patch::{compile, parse_patch, validate};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{RenderConfig, render};

/// The patch id of a sequence written for no particular patch.
pub const PATCH_ID: &str = "7f0c0000-0000-4000-8000-0000000009e1";

/// The eight physical parameters of the voice, in § 5.2 order (times in seconds, cutoff in Hz,
/// drive and gain in dB).
#[derive(Clone, Copy, Debug)]
pub struct Voice {
    pub attack: f64,
    pub decay: f64,
    pub sustain: f64,
    pub release: f64,
    pub cutoff: f64,
    pub q: f64,
    pub drive: f64,
    pub gain: f64,
}

/// A plain voice: short attack, a sustain below the peak, a short release, an open filter.
pub const PLAIN: Voice = Voice {
    attack: 0.005,
    decay: 0.08,
    sustain: 0.7,
    release: 0.03,
    cutoff: 6_000.0,
    q: 0.7,
    drive: 0.0,
    gain: -6.0,
};

/// One note: start, gate end, MIDI number, velocity.
pub type Note = (u64, u64, u8, f32);

pub fn sequence(notes: &[Note]) -> NoteSequence {
    NoteSequence {
        format_version: 1,
        sample_rate_hz: 48_000,
        patch_id: PATCH_ID.to_owned(),
        note_estimation_id: None,
        notes: notes
            .iter()
            .map(
                |&(start_sample, end_sample, midi_note, velocity)| NoteEvent {
                    start_sample,
                    end_sample,
                    midi_note,
                    velocity,
                },
            )
            .collect(),
    }
}

/// The voice's samples for `notes`: the last note's end, its release and the renderer's tail.
pub fn render_notes(voice: Voice, notes: &[Note]) -> Vec<f32> {
    let text = format!(
        "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
         node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive {}\n\
         node 4 LowpassFilter\nparam 4 filter_cutoff {}\nparam 4 resonance_q {}\n\
         node 5 AdsrEnvelope\nparam 5 attack {}\nparam 5 decay {}\nparam 5 sustain {}\n\
         param 5 release {}\nnode 6 Vca\nnode 7 Output\nparam 7 output_gain {}\n\
         edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
         edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
         edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n",
        voice.drive,
        voice.cutoff,
        voice.q,
        voice.attack,
        voice.decay,
        voice.sustain,
        voice.release,
        voice.gain
    );
    let plan = compile(&validate(&parse_patch(&text).unwrap()).unwrap());
    render(&plan, &sequence(notes), &RenderConfig::default())
        .unwrap()
        .samples
}

/// The f0 of a MIDI number in equal temperament, A4 = 440 Hz.
pub fn midi_hz(midi: u8) -> f64 {
    440.0 * 2f64.powf((f64::from(midi) - 69.0) / 12.0)
}

/// The signed distance from `hz` to `midi`'s pitch in cents.
pub fn cents(hz: f64, midi: u8) -> f64 {
    1_200.0 * (hz / midi_hz(midi)).log2()
}

/// A band-limited square wave at `midi` (odd harmonics below 20 kHz), `seconds` long, at
/// `amplitude`, with 5 ms linear fades and `pad` silent samples before and after. The voice has
/// no square oscillator, so the square of Dc1-3 is synthesized here.
pub fn square(midi: u8, seconds: f64, amplitude: f64, pad: usize) -> Vec<f32> {
    let f0 = midi_hz(midi);
    let count = (seconds * 48_000.0) as usize;
    let fade = 240.0;
    let mut out = vec![0.0f32; pad];
    for n in 0..count {
        let t = n as f64 / 48_000.0;
        let mut value = 0.0;
        let mut k = 1.0;
        while k * f0 < 20_000.0 {
            value += (2.0 * std::f64::consts::PI * k * f0 * t).sin() / k;
            k += 2.0;
        }
        let ramp = (n as f64 / fade).min((count - n) as f64 / fade).min(1.0);
        out.push((amplitude * ramp * value * 4.0 / std::f64::consts::PI) as f32);
    }
    out.extend(std::iter::repeat_n(0.0, pad));
    out
}

/// The RMS of the 1,024 samples centred on `t` (`[t - 512, t + 512)`, zero outside the clip).
pub fn centred_rms(samples: &[f32], t: usize) -> f64 {
    let start = t.saturating_sub(512);
    let end = (t + 512).min(samples.len());
    let sum: f64 = samples[start.min(end)..end]
        .iter()
        .map(|s| f64::from(*s) * f64::from(*s))
        .sum();
    (sum / 1_024.0).sqrt()
}

/// The workers running in this test process, and the signal that one has finished.
static WORKERS: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());

/// `f` of every job, in job order, computed by a pool of workers. However many tests call it at
/// once, the process runs at most [`std::thread::available_parallelism`] workers (which follows
/// the affinity mask on Linux): a call waits for one free worker slot and takes the free ones up
/// to its number of jobs. Each job is computed on its own, so the results do not depend on the
/// number of workers.
pub fn pool_map<T: Sync, R: Send>(jobs: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    if jobs.is_empty() {
        return Vec::new();
    }
    let capacity = std::thread::available_parallelism().map_or(1, |n| n.get());
    let (lock, freed) = &WORKERS;
    let taken = {
        let mut running = lock.lock().unwrap();
        while *running >= capacity {
            running = freed.wait(running).unwrap();
        }
        let taken = (capacity - *running).min(jobs.len());
        *running += taken;
        taken
    };
    let results: Vec<Mutex<Option<R>>> = jobs.iter().map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..taken {
            scope.spawn(|| {
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(k) else {
                        break;
                    };
                    *results[k].lock().unwrap() = Some(f(job));
                }
            });
        }
    });
    *lock.lock().unwrap() -= taken;
    freed.notify_all();
    results
        .into_iter()
        .map(|slot| slot.into_inner().unwrap().unwrap())
        .collect()
}
