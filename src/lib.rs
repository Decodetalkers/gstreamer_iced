mod gstreamer_pipewire;
mod gstreamer_playbin;
mod id;
mod pipeline;
mod video_player;

use gst::glib;
use gst::prelude::*;
use gst::GenericFormattedValue;
use gstreamer as gst;
use iced_widget::image;
use std::hash::Hash;
use std::os::fd::RawFd;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
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

impl FrameData {
    fn stride(&self) -> u32 {
        self.width
    }
    fn data(&self) -> &[u8] {
        &self.pixels
    }
    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
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
    pub handle: Option<image::Handle>,
    pub duration: std::time::Duration,
    pub position: std::time::Duration,
    pub volume: f64,
    pub get_duration_attempt: bool,
}
impl State {
    fn new() -> Self {
        Self::default()
    }
    fn with_try_get_duration(self, info_get_started: bool) -> Self {
        Self {
            get_duration_attempt: info_get_started,
            ..self
        }
    }
}
#[derive(Debug)]
pub enum GVideo {
    UrlPlayer(GVideoUrl),
    PipeWire(GVideoPipewire),
    None,
}

impl GVideo {
    pub fn new() -> Self {
        Self::None
    }
    pub fn open_pipewire(&mut self, path: u32, fd: RawFd) -> Result<(), IcedGStreamerError> {
        *self = Self::new_pipewire(path, fd)?;
        Ok(())
    }
    pub fn new_pipewire(path: u32, fd: RawFd) -> Result<Self, IcedGStreamerError> {
        Ok(Self::PipeWire(GVideoPipewire::new_pipewire(path, fd)?))
    }
    pub fn new_url(url: &url::Url, is_live: bool) -> Result<Self, IcedGStreamerError> {
        Ok(Self::UrlPlayer(GVideoUrl::new_url(url, is_live)?))
    }
    pub fn as_url(&self) -> &GVideoUrl {
        let Self::UrlPlayer(url_player) = &self else {
            panic!("Not this type");
        };
        url_player
    }
    fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
    fn stream_type(&self) -> StreamType {
        match self {
            Self::None => StreamType::Empty,
            Self::PipeWire(_) => StreamType::PipeWire,
            Self::UrlPlayer(_) => StreamType::UrlPlayer,
        }
    }
    fn frame_data(&self) -> Option<FrameData> {
        match self {
            Self::None => None,
            Self::UrlPlayer(player) => player.frame_data(),
            Self::PipeWire(pipewire) => pipewire.frame_data(),
        }
    }

    fn upload_frame(&self) -> Option<Arc<AtomicBool>> {
        match self {
            Self::None => None,
            Self::UrlPlayer(player) => Some(player.upload_frame.clone()),
            Self::PipeWire(pipewire) => Some(pipewire.upload_frame.clone()),
        }
    }
    fn alive(&self) -> Option<Arc<AtomicBool>> {
        match self {
            Self::None => None,
            Self::UrlPlayer(player) => Some(player.alive.clone()),
            Self::PipeWire(pipewire) => Some(pipewire.alive.clone()),
        }
    }
    fn id(&self) -> Option<id::Id> {
        match self {
            Self::None => None,
            Self::UrlPlayer(player) => Some(player.id),
            Self::PipeWire(pipewire) => Some(pipewire.id),
        }
    }
    fn frame(&self) -> Option<Arc<Mutex<Option<FrameData>>>> {
        match self {
            Self::None => None,
            Self::UrlPlayer(player) => Some(player.frame.clone()),
            Self::PipeWire(pipewire) => Some(pipewire.frame.clone()),
        }
    }
    fn state(&self) -> Option<Arc<RwLock<State>>> {
        match self {
            Self::None => None,
            Self::UrlPlayer(player) => Some(player.state.clone()),
            Self::PipeWire(pipewire) => Some(pipewire.state.clone()),
        }
    }
    pub fn play_state(&self) -> gst::State {
        match self {
            Self::None => gst::State::Null,
            Self::UrlPlayer(player) => player.play_state(),
            Self::PipeWire(pipewire) => pipewire.play_state(),
        }
    }
    fn source(&self) -> Option<&gst::Bin> {
        match self {
            Self::None => None,
            Self::UrlPlayer(player) => Some(&player.source),
            Self::PipeWire(pipewire) => Some(&pipewire.source),
        }
    }
    fn bus(&self) -> Option<&gst::Bus> {
        match self {
            Self::None => None,
            Self::UrlPlayer(player) => Some(&player.bus),
            Self::PipeWire(pipewire) => Some(&pipewire.bus),
        }
    }
    pub fn set_state(&self, state: gst::State) {
        match self {
            Self::None => {}
            Self::UrlPlayer(player) => {
                player.set_state(state);
            }
            Self::PipeWire(pipewire) => {
                pipewire.set_state(state);
            }
        }
    }
}

#[derive(Debug)]
pub struct GVideoInner<const X: usize> {
    bus: gst::Bus,
    source: gst::Bin,
    state: Arc<RwLock<State>>,
    upload_frame: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    frame: Arc<Mutex<Option<FrameData>>>,
    id: id::Id,
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

impl<const X: usize> Drop for GVideoInner<X> {
    fn drop(&mut self) {
        self.source
            .set_state(gst::State::Null)
            .expect("failed to set state");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamType {
    UrlPlayer,
    PipeWire,
    Empty,
}

impl<const X: usize> GVideoInner<X> {
    /// return an [image::Handle], you can use it to make image
    pub fn frame_handle(&self) -> Option<image::Handle> {
        self.frame_data().map(|frame| frame.into())
    }

    /// return [FrameData], you can directly access the data
    pub fn frame_data(&self) -> Option<FrameData> {
        self.frame.lock().map(|frame| frame.clone()).unwrap_or(None)
    }

    /// what the playing status is
    pub fn play_state(&self) -> gst::State {
        self.source.current_state()
    }

    pub fn is_playing(&self) -> bool {
        matches!(self.play_state(), gst::State::Playing)
    }

    /// get the type name
    pub fn stream_type(&self) -> StreamType {
        match X {
            0 => StreamType::UrlPlayer,
            1 => StreamType::PipeWire,
            _ => unreachable!(),
        }
    }
    pub fn set_state(&self, state: PlayingState) {
        match state {
            PlayingState::Playing | PlayingState::Paused => {
                self.source.set_state(state).unwrap();
            }
            _ => {}
        }
    }
}
