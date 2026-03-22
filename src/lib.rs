mod gstreamer_pipewire;
mod gstreamer_playbin;
mod video_player;

use gst::glib;
use gst::prelude::*;
use gst::GenericFormattedValue;
use gstreamer as gst;
use iced_widget::image;
use std::hash::Hash;
use std::sync::{Arc, RwLock};
use thiserror::Error;
pub mod reexport {
    pub use url;
}
pub use video_player::VideoPlayer;

pub use gst::State as PlayingState;
#[derive(Debug, Clone)]
pub struct FrameData {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl From<FrameData> for image::Handle {
    fn from(
        FrameData {
            pixels,
            width,
            height,
        }: FrameData,
    ) -> Self {
        image::Handle::from_rgba(width, height, pixels)
    }
}

pub use gstreamer_playbin::GVideoUrl;

pub use gstreamer_pipewire::GVideoPipewire;

#[derive(Debug, Default)]
struct State {
    pub frame: Option<FrameData>,
    pub duration: std::time::Duration,
    pub position: std::time::Duration,
    pub volume: f64,
    pub info_get_started: bool,
}
impl State {
    fn new() -> Self {
        Self::default()
    }
    fn with_info_get_started(self, info_get_started: bool) -> Self {
        Self {
            info_get_started,
            ..self
        }
    }
}

#[derive(Debug)]
pub struct GVideo<const X: usize> {
    bus: gst::Bus,
    source: gst::Bin,
    state: Arc<RwLock<State>>,
}

#[derive(Debug, Error)]
pub enum IcedGStreamerError {
    #[error("{0}")]
    Glib(#[from] glib::Error),
    #[error("{0}")]
    Bool(#[from] glib::BoolError),
    #[error("failed to get the gstreamer bus")]
    Bus,
    #[error("{0}")]
    StateChange(#[from] gst::StateChangeError),
    #[error("failed to cast gstreamer element")]
    Cast,
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("invalid URI")]
    Uri,
    #[error("failed to get media capabilities")]
    Caps,
    #[error("failed to query media duration or position")]
    Duration,
    #[error("failed to sync with playback")]
    Sync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Position {
    /// Position based on time.
    ///
    /// Not the most accurate format for videos.
    Time(std::time::Duration),
    /// Position based on nth frame.
    Frame(u64),
}

impl From<Position> for GenericFormattedValue {
    fn from(pos: Position) -> Self {
        match pos {
            Position::Time(t) => gst::ClockTime::from_nseconds(t.as_nanos() as _).into(),
            Position::Frame(f) => gst::format::Default::from_u64(f).into(),
        }
    }
}

impl From<std::time::Duration> for Position {
    fn from(t: std::time::Duration) -> Self {
        Position::Time(t)
    }
}

impl From<u64> for Position {
    fn from(f: u64) -> Self {
        Position::Frame(f)
    }
}

impl<const X: usize> Drop for GVideo<X> {
    fn drop(&mut self) {
        self.source
            .set_state(gst::State::Null)
            .expect("failed to set state");
    }
}

impl<const X: usize> GVideo<X> {
    /// return an [image::Handle], you can use it to make image
    pub fn frame_handle(&self) -> Option<image::Handle> {
        self.state
            .read()
            .map(|state| state.frame.clone())
            .map(|frame| frame.map(|f| f.into()))
            .unwrap_or(None)
    }

    /// return [FrameData], you can directly access the data
    pub fn frame_data(&self) -> Option<FrameData> {
        self.state
            .read()
            .map(|state| state.frame.clone())
            .unwrap_or(None)
    }

    /// what the playing status is
    pub fn play_state(&self) -> gst::State {
        self.source.current_state()
    }

    pub fn is_playing(&self) -> bool {
        matches!(self.play_state(), gst::State::Playing)
    }

    /// get the type name
    pub fn gstreamer_type(&self) -> &str {
        match X {
            0 => "base",
            1 => "pipewire",
            _ => unreachable!(),
        }
    }
    pub fn set_status(&self, state: PlayingState) {
        match state {
            PlayingState::Playing | PlayingState::Paused => {
                self.source.set_state(state).unwrap();
            }
            _ => {}
        }
    }
}
