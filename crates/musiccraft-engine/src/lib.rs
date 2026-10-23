//! Real-time audio engine.
//!
//! The engine is driven entirely by [`EngineMessage`] values and never reads the
//! project model. Everything reachable from [`Engine::process`] must stay free of
//! allocation, deallocation, locks, I/O, and panics.
//!
//! Small changes (tempo, volumes, effect parameters) travel as plain `Copy` data.
//! Large data ([`Graph`], [`Sequence`]) is allocated on the main thread, moved to
//! the audio thread inside a message, and the value it replaces is moved back
//! through a return queue so that it is dropped on the main thread.

mod controllers;
mod engine;
mod external;
mod graph;
mod message;
mod meters;
mod midi_out;
mod sequence;

pub use controllers::{ControllerLane, ControllerSource, Convert, MAX_CONTROLLERS};
pub use engine::{Engine, EngineHandle, LiveSender, engine_pair};
pub use external::{External, ExternalTransport, MAX_EXTERNALS};
pub use graph::{
    ChannelNode, ChannelParams, EffectParams, Graph, Instrument, MAX_SENDS, Preview, SendParams,
    TrackParams, Zone,
};
pub use message::{EngineMessage, Garbage, LiveEvent, Start};
pub use meters::{ACTIVITY_CHANNELS, Meters, SCOPE_LEN};
pub use midi_out::{MIDI_OUT_CAPACITY, MidiOutEvent, MidiOutReceiver};
pub use musiccraft_dsp::PanLaw;
pub use musiccraft_meter::{Meter, TimeSig};
pub use sequence::{AutomationLane, AutomationTarget, NoteEvent, NoteShape, Sequence};

/// Largest number of frames processed in one internal block. Device callbacks of
/// any size are split into blocks of at most this many frames.
pub const MAX_BLOCK: usize = 256;
/// Size of the voice pool. When every voice is busy the oldest is stolen.
pub const MAX_VOICES: usize = 64;
/// Capacity of the main-thread → engine message queue.
pub const MESSAGE_CAPACITY: usize = 4096;
/// Capacity of the engine → main-thread return queue.
pub const GARBAGE_CAPACITY: usize = 64;
/// Capacity of the live (MIDI / preview) note queue.
pub const LIVE_CAPACITY: usize = 1024;

#[cfg(test)]
mod tests;
