//! The offline renderer (DESIGN.md § 5.1, § 5.3): a compiled plan of the fixed
//! `subtractive-mono/1` voice, driven by a note sequence, rendered block by block at 48 kHz.

use std::fmt;

use patchback_audio::MAX_FRAMES;
use patchback_core::parameter::Topology;
use patchback_core::patch::{ExecutionPlan, NodeKind, PlanStep};
use patchback_core::score::{EventsFileReason, NoteSequence};

use crate::{
    Adsr, AdsrStage, Drive, OutputStage, PolyBlepSaw, SAMPLE_RATE_HZ, SMOOTHING_SECONDS, Smoother,
    SvfLowpass, midi_to_hz,
};

/// The block size when none is given, in frames (§ 5.3).
pub const DEFAULT_BLOCK_FRAMES: usize = 256;

/// The frames rendered after the last note's release has ended: 100 ms at 48 kHz.
///
/// The filter precedes the VCA, so these frames are silence: the envelope is 0 once the release
/// has ended. The constant exists so a clip carries the full release and a margin after it for
/// analysis windows that run past a note's end (`RELEASE_WINDOW_FRAMES` of `patchback-analysis`,
/// 9600 frames from the note end; a window longer than the release and this tail is cut at the
/// clip's end).
///
/// Provisional: DESIGN.md names no render length; § 17 leaves it open.
pub const RENDER_TAIL_FRAMES: u64 = 4_800;

/// How a render runs: the block size in frames, as the operator gave it. Only a positive size
/// renders ([`RenderError::InvalidBlockSize`]); there is no upper bound beyond the type's, and a
/// block longer than the render is the render.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderConfig {
    pub block_size_frames: i64,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            block_size_frames: DEFAULT_BLOCK_FRAMES as i64,
        }
    }
}

/// What a render reports beside its samples (§ 5.3: limiting is reported, never silent).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderReport {
    /// The number of samples rendered, [`render_frame_count`].
    pub frame_count: usize,
    /// The block size the render ran with.
    pub block_size_frames: usize,
    /// Whether the protective limiter changed any sample.
    pub limited: bool,
    /// How many samples it changed.
    pub limited_samples: u64,
}

/// A successful render: mono samples at 48 kHz, each finite and within the limiter threshold.
#[derive(Clone, Debug, PartialEq)]
pub struct Rendered {
    pub samples: Vec<f32>,
    pub report: RenderReport,
}

/// Why a render produced nothing. Each names an error of `spec/domains/audio.yaml`
/// ([`Self::domain_error`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// `patchback.audio.UnsupportedSampleRate`: the note sequence is not at 48 kHz.
    UnsupportedSampleRate { sample_rate_hz: u32 },
    /// `patchback.audio.InvalidBlockSize`: the block size is not a positive number of frames.
    InvalidBlockSize { block_size_frames: i64 },
    /// `patchback.audio.NoteSequenceNotFound`: a note does not end after it starts, which the
    /// sequence loader refuses (`patchback.score.InvalidNoteTiming`), so no loadable sequence is
    /// the one given; the CLI answers a `notes.events` file that does not load the same way.
    InvalidNoteTiming { start_sample: u64, end_sample: u64 },
    /// `patchback.audio.InputTooLong`: the render would be longer than
    /// [`patchback_audio::MAX_FRAMES`], the bound of every clip. Nothing was rendered.
    RenderTooLong { frame_count: u64 },
    /// `patchback.audio.NonFiniteRender`: the sample at `frame` is NaN or infinite. Nothing is
    /// kept.
    NonFiniteRender { frame: u64 },
}

impl RenderError {
    /// Every error of `spec/domains/audio.yaml` the renderer can emit.
    pub const DOMAIN_ERRORS: &'static [&'static str] = &[
        "patchback.audio.UnsupportedSampleRate",
        "patchback.audio.InvalidBlockSize",
        "patchback.audio.NoteSequenceNotFound",
        "patchback.audio.InputTooLong",
        "patchback.audio.NonFiniteRender",
    ];

    /// The error of `spec/domains/audio.yaml` this refusal is.
    pub fn domain_error(&self) -> &'static str {
        match self {
            Self::UnsupportedSampleRate { .. } => "patchback.audio.UnsupportedSampleRate",
            Self::InvalidBlockSize { .. } => "patchback.audio.InvalidBlockSize",
            Self::InvalidNoteTiming { .. } => "patchback.audio.NoteSequenceNotFound",
            Self::RenderTooLong { .. } => "patchback.audio.InputTooLong",
            Self::NonFiniteRender { .. } => "patchback.audio.NonFiniteRender",
        }
    }

    /// One value of every variant. The `match` below has no wildcard, so a new variant does not
    /// compile until it is listed here, and the domain test then checks its name.
    pub fn examples() -> Vec<RenderError> {
        let examples = vec![
            Self::UnsupportedSampleRate {
                sample_rate_hz: 44_100,
            },
            Self::InvalidBlockSize {
                block_size_frames: 0,
            },
            Self::InvalidNoteTiming {
                start_sample: 0,
                end_sample: 0,
            },
            Self::RenderTooLong { frame_count: 0 },
            Self::NonFiniteRender { frame: 0 },
        ];
        for example in &examples {
            match example {
                Self::UnsupportedSampleRate { .. }
                | Self::InvalidBlockSize { .. }
                | Self::InvalidNoteTiming { .. }
                | Self::RenderTooLong { .. }
                | Self::NonFiniteRender { .. } => {}
            }
        }
        examples
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSampleRate { sample_rate_hz } => write!(
                f,
                "sample rate {sample_rate_hz} Hz; the renderer runs at {SAMPLE_RATE_HZ} Hz only and converts nothing"
            ),
            Self::InvalidBlockSize { block_size_frames } => write!(
                f,
                "block size {block_size_frames} frames; a block is a positive number of frames"
            ),
            Self::InvalidNoteTiming {
                start_sample,
                end_sample,
            } => {
                let reason = EventsFileReason::InvalidNoteTiming {
                    start_sample: *start_sample,
                    end_sample: *end_sample,
                };
                write!(
                    f,
                    "the sequence loader refuses it: {}: {reason}",
                    reason.spec_error()
                )
            }
            Self::RenderTooLong { frame_count } => write!(
                f,
                "the render would be {frame_count} frames, past the bound of {MAX_FRAMES} frames"
            ),
            Self::NonFiniteRender { frame } => {
                write!(
                    f,
                    "the render produced a non-finite sample at frame {frame}"
                )
            }
        }
    }
}

impl std::error::Error for RenderError {}

/// The length of a render, in frames: the end of the note that ends last (0 without notes), plus
/// the release time of the plan's envelope in samples as the envelope runs it
/// ([`Adsr::release_samples`]), plus [`RENDER_TAIL_FRAMES`]. Saturates at `u64::MAX`.
pub fn render_frame_count(plan: &ExecutionPlan, notes: &NoteSequence) -> u64 {
    let last_end = notes.notes.iter().map(|n| n.end_sample).max().unwrap_or(0);
    let release = plan
        .steps()
        .iter()
        .find(|step| step.kind == NodeKind::AdsrEnvelope)
        .map(|step| envelope(step).release_samples())
        .unwrap_or(1);
    last_end
        .saturating_add(release)
        .saturating_add(RENDER_TAIL_FRAMES)
}

/// Renders `plan` driven by `notes`.
///
/// # Semantics
///
/// - The plan is walked step by step in its order for each block of up to
///   `config.block_size_frames` frames; each step reads the buffers its inputs name and writes
///   its own (§ 5.1). Every unit runs sample by sample and events carry their sample offset, so
///   the block size changes how the work is cut and never the samples.
/// - The voice is monophonic. A note starts at `start_sample` and its gate closes at
///   `end_sample`; at one sample a note-off is applied before a note-on, so a note starting where
///   the last ends retriggers the envelope from its current level. A note-off of a note that is
///   no longer the sounding one (a later note started) is ignored.
/// - The oscillator follows the sounding note's pitch, holds it through the release, and before
///   the first note runs at the first note's pitch. Its phase is never reset. A MIDI number above
///   127 is 127.
/// - Of what the sequence loader refuses, only a note that does not end after it starts is
///   refused here (Refusals); overlapping notes, MIDI numbers and velocities out of range render
///   as this list says.
/// - Velocity scales the envelope; a velocity outside `[0, 1]` is clamped (NaN is 0). It changes
///   at once when the envelope is idle and is smoothed over [`SMOOTHING_SECONDS`] otherwise.
/// - The sequence's recorded patch is not compared with the plan: any patch at the sequence's
///   rate may render it (`spec/domains/audio.yaml` `RenderPatch`).
///
/// # Refusals
///
/// In this order, each producing nothing: a sequence not at 48 kHz
/// ([`RenderError::UnsupportedSampleRate`]); a block size of zero or less
/// ([`RenderError::InvalidBlockSize`]); a note, the first in the sequence's order, that does not
/// end after it starts ([`RenderError::InvalidNoteTiming`], as the sequence loader refuses it);
/// a length past [`patchback_audio::MAX_FRAMES`]
/// ([`RenderError::RenderTooLong`]), before anything is allocated; a non-finite output sample
/// ([`RenderError::NonFiniteRender`]), at the first one.
pub fn render(
    plan: &ExecutionPlan,
    notes: &NoteSequence,
    config: &RenderConfig,
) -> Result<Rendered, RenderError> {
    if notes.sample_rate_hz != SAMPLE_RATE_HZ {
        return Err(RenderError::UnsupportedSampleRate {
            sample_rate_hz: notes.sample_rate_hz,
        });
    }
    if config.block_size_frames <= 0 {
        return Err(RenderError::InvalidBlockSize {
            block_size_frames: config.block_size_frames,
        });
    }
    if let Some(note) = notes
        .notes
        .iter()
        .find(|note| note.end_sample <= note.start_sample)
    {
        return Err(RenderError::InvalidNoteTiming {
            start_sample: note.start_sample,
            end_sample: note.end_sample,
        });
    }
    let block_size = usize::try_from(config.block_size_frames).unwrap_or(usize::MAX);
    let frame_count = render_frame_count(plan, notes);
    let frames = match usize::try_from(frame_count) {
        Ok(frames) if frames <= MAX_FRAMES => frames,
        _ => return Err(RenderError::RenderTooLong { frame_count }),
    };

    let mut voice = Voice::new(plan, notes);
    let mut buffers: Vec<Buffer> = (0..plan.buffer_count())
        .map(|_| Buffer::default())
        .collect();
    let mut samples: Vec<f32> = Vec::with_capacity(frames);
    let mut start = 0;
    while start < frames {
        let length = block_size.min(frames - start);
        voice.process_block(plan, &mut buffers, start, length, &mut samples)?;
        start += length;
    }
    let limited_samples = voice.limited_samples();
    Ok(Rendered {
        samples,
        report: RenderReport {
            frame_count: frames,
            block_size_frames: block_size,
            limited: limited_samples > 0,
            limited_samples,
        },
    })
}

/// A gate event of one block, at its offset from the block's first frame.
#[derive(Clone, Copy, Debug, PartialEq)]
struct GateEvent {
    offset: usize,
    kind: Gate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Gate {
    On { velocity: f64 },
    Off,
}

/// A buffer of one block: samples for an Audio or Control port, gate events for an Event port.
#[derive(Debug)]
enum Buffer {
    Signal(Vec<f64>),
    Gate(Vec<GateEvent>),
}

impl Default for Buffer {
    fn default() -> Self {
        Self::Signal(Vec::new())
    }
}

impl Buffer {
    fn signal(&mut self) -> &mut Vec<f64> {
        if !matches!(self, Self::Signal(_)) {
            *self = Self::Signal(Vec::new());
        }
        match self {
            Self::Signal(samples) => {
                samples.clear();
                samples
            }
            Self::Gate(_) => unreachable!("set to a signal above"),
        }
    }

    fn gate(&mut self) -> &mut Vec<GateEvent> {
        if !matches!(self, Self::Gate(_)) {
            *self = Self::Gate(Vec::new());
        }
        match self {
            Self::Gate(events) => {
                events.clear();
                events
            }
            Self::Signal(_) => unreachable!("set to a gate above"),
        }
    }

    /// The sample at `index`; 0 for an event buffer or past the end.
    fn sample(&self, index: usize) -> f64 {
        match self {
            Self::Signal(samples) => samples.get(index).copied().unwrap_or(0.0),
            Self::Gate(_) => 0.0,
        }
    }

    fn events(&self) -> &[GateEvent] {
        match self {
            Self::Gate(events) => events,
            Self::Signal(_) => &[],
        }
    }
}

/// A note boundary of the sequence, at its frame.
#[derive(Clone, Copy, Debug)]
struct NoteBoundary {
    frame: u64,
    /// The note's position in start order.
    note: usize,
    on: Option<(u8, f64)>,
}

/// The `NoteInput` node: the sequence as a pitch signal and gate events.
struct NoteInput {
    boundaries: Vec<NoteBoundary>,
    next: usize,
    sounding: Option<usize>,
    pitch_hz: f64,
}

impl NoteInput {
    fn new(notes: &NoteSequence) -> Self {
        let mut ordered = notes.notes.clone();
        ordered.sort_by_key(|note| note.start_sample);
        let mut boundaries = Vec::with_capacity(ordered.len() * 2);
        for (index, note) in ordered.iter().enumerate() {
            let velocity = if note.velocity.is_nan() {
                0.0
            } else {
                f64::from(note.velocity).clamp(0.0, 1.0)
            };
            boundaries.push(NoteBoundary {
                frame: note.start_sample,
                note: index,
                on: Some((note.midi_note.min(127), velocity)),
            });
            boundaries.push(NoteBoundary {
                frame: note.end_sample,
                note: index,
                on: None,
            });
        }
        // At one frame, note-offs before note-ons; otherwise in start order.
        boundaries.sort_by_key(|b| (b.frame, b.on.is_some(), b.note));
        Self {
            boundaries,
            next: 0,
            sounding: None,
            pitch_hz: ordered
                .first()
                .map(|note| midi_to_hz(note.midi_note.min(127)))
                .unwrap_or(0.0),
        }
    }

    fn process(
        &mut self,
        start: usize,
        length: usize,
        pitch: &mut Vec<f64>,
        gate: &mut Vec<GateEvent>,
    ) {
        for offset in 0..length {
            let frame = (start + offset) as u64;
            while let Some(boundary) = self
                .boundaries
                .get(self.next)
                .copied()
                .filter(|b| b.frame <= frame)
            {
                self.next += 1;
                match boundary.on {
                    Some((midi_note, velocity)) => {
                        self.sounding = Some(boundary.note);
                        self.pitch_hz = midi_to_hz(midi_note);
                        gate.push(GateEvent {
                            offset,
                            kind: Gate::On { velocity },
                        });
                    }
                    None if self.sounding == Some(boundary.note) => {
                        self.sounding = None;
                        gate.push(GateEvent {
                            offset,
                            kind: Gate::Off,
                        });
                    }
                    None => {}
                }
            }
            pitch.push(self.pitch_hz);
        }
    }
}

/// The `AdsrEnvelope` node: the envelope times the smoothed velocity.
struct Envelope {
    adsr: Adsr,
    velocity: Smoother,
}

/// One node of the plan with its state (§ 5.3: each voice owns its state).
enum Node {
    NoteInput(NoteInput),
    Saw(PolyBlepSaw),
    Drive(Drive),
    Lowpass(SvfLowpass),
    Envelope(Envelope),
    Vca,
    Output(OutputStage),
}

/// The physical value of a node parameter; a parameter the node does not carry is its contract's
/// default, which is the placeholder of DESIGN.md § 17 OQ-1. Validation refuses a node that lacks
/// one, so a compiled plan always carries them.
fn parameter(step: &PlanStep, path: &str) -> f64 {
    step.parameters
        .iter()
        .find(|p| p.name == path)
        .map(|p| p.value)
        .or_else(|| {
            Topology::subtractive_mono()
                .contract(path)
                .map(|contract| contract.default_physical())
        })
        .unwrap_or(0.0)
}

fn envelope(step: &PlanStep) -> Adsr {
    Adsr::new(
        parameter(step, "attack"),
        parameter(step, "decay"),
        parameter(step, "sustain"),
        parameter(step, "release"),
    )
}

/// The nodes of a plan, one per step, in the plan's order.
struct Voice {
    nodes: Vec<Node>,
}

impl Voice {
    fn new(plan: &ExecutionPlan, notes: &NoteSequence) -> Self {
        let nodes = plan
            .steps()
            .iter()
            .map(|step| match step.kind {
                NodeKind::NoteInput => Node::NoteInput(NoteInput::new(notes)),
                NodeKind::SawOscillator => Node::Saw(PolyBlepSaw::new()),
                NodeKind::Drive => Node::Drive(Drive::new(parameter(step, "drive"))),
                NodeKind::LowpassFilter => Node::Lowpass(SvfLowpass::new(
                    parameter(step, "filter_cutoff"),
                    parameter(step, "resonance_q"),
                )),
                NodeKind::AdsrEnvelope => Node::Envelope(Envelope {
                    adsr: envelope(step),
                    velocity: Smoother::new(0.0, SMOOTHING_SECONDS),
                }),
                NodeKind::Vca => Node::Vca,
                NodeKind::Output => Node::Output(OutputStage::new(parameter(step, "output_gain"))),
                kind => unreachable!(
                    "a compiled subtractive-mono/1 plan holds only the § 5.2 kinds, not {}",
                    kind.name()
                ),
            })
            .collect();
        Self { nodes }
    }

    fn limited_samples(&self) -> u64 {
        self.nodes
            .iter()
            .map(|node| match node {
                Node::Output(stage) => stage.limited_samples(),
                _ => 0,
            })
            .sum()
    }

    /// Runs every step over the block of `length` frames from frame `start`.
    fn process_block(
        &mut self,
        plan: &ExecutionPlan,
        buffers: &mut [Buffer],
        start: usize,
        length: usize,
        samples: &mut Vec<f32>,
    ) -> Result<(), RenderError> {
        for (step, node) in plan.steps().iter().zip(&mut self.nodes) {
            let mut outputs: Vec<Buffer> = step
                .outputs
                .iter()
                .map(|output| std::mem::take(&mut buffers[output.buffer]))
                .collect();
            let input = |port: &str| {
                step.inputs
                    .iter()
                    .find(|input| input.port == port)
                    .map(|input| &buffers[input.buffer])
            };
            let output = |port: &str| step.outputs.iter().position(|output| output.port == port);
            match node {
                Node::NoteInput(notes) => {
                    let (Some(pitch), Some(gate)) = (output("pitch"), output("gate")) else {
                        unreachable!("NoteInput declares pitch and gate")
                    };
                    let mut pitch_buffer = std::mem::take(&mut outputs[pitch]);
                    notes.process(start, length, pitch_buffer.signal(), outputs[gate].gate());
                    outputs[pitch] = pitch_buffer;
                }
                Node::Saw(saw) => {
                    let pitch = input("pitch");
                    let out = single(&mut outputs).signal();
                    for i in 0..length {
                        if let Some(pitch) = pitch {
                            saw.set_frequency(pitch.sample(i));
                        }
                        out.push(saw.next_sample());
                    }
                }
                Node::Drive(drive) => {
                    let input = input("in");
                    let out = single(&mut outputs).signal();
                    for i in 0..length {
                        out.push(drive.process(input.map_or(0.0, |b| b.sample(i))));
                    }
                }
                Node::Lowpass(filter) => {
                    let input = input("in");
                    let out = single(&mut outputs).signal();
                    for i in 0..length {
                        out.push(filter.process(input.map_or(0.0, |b| b.sample(i))));
                    }
                }
                Node::Envelope(envelope) => {
                    let events = input("gate").map_or(&[][..], Buffer::events);
                    let out = single(&mut outputs).signal();
                    let mut next = 0;
                    for i in 0..length {
                        while let Some(event) = events.get(next).filter(|e| e.offset == i) {
                            next += 1;
                            match event.kind {
                                Gate::On { velocity } => {
                                    if envelope.adsr.stage() == AdsrStage::Idle {
                                        envelope.velocity.jump_to(velocity);
                                    } else {
                                        envelope.velocity.set_target(velocity);
                                    }
                                    envelope.adsr.note_on();
                                }
                                Gate::Off => envelope.adsr.note_off(),
                            }
                        }
                        out.push(envelope.adsr.next_sample() * envelope.velocity.next_sample());
                    }
                }
                Node::Vca => {
                    let (input, gain) = (input("in"), input("gain"));
                    let out = single(&mut outputs).signal();
                    for i in 0..length {
                        let x = input.map_or(0.0, |b| b.sample(i));
                        out.push(x * gain.map_or(0.0, |b| b.sample(i)));
                    }
                }
                Node::Output(stage) => {
                    let input = input("in");
                    for i in 0..length {
                        let frame = (start + i) as u64;
                        samples.push(stage.process(frame, input.map_or(0.0, |b| b.sample(i)))?);
                    }
                }
            }
            for (output, buffer) in step.outputs.iter().zip(outputs) {
                buffers[output.buffer] = buffer;
            }
        }
        Ok(())
    }
}

/// The one output buffer of a node kind with one output port.
fn single(outputs: &mut [Buffer]) -> &mut Buffer {
    match outputs {
        [only] => only,
        _ => unreachable!("the kind declares one output port"),
    }
}
