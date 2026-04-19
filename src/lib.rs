mod gstreamer_pipewire;
mod gstreamer_playbin;
mod id;
mod pipeline;
mod video_player;

use gst::GenericFormattedValue;
use gst::glib;
use gst::prelude::*;
use gstreamer as gst;
use std::hash::Hash;
use std::os::fd::RawFd;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, RwLock};
use thiserror::Error;
pub mod reexport {
    pub use url;
}
pub use video_player::VideoPlayer;

pub use gst::State as PlayingState;

/// the data of the frame
/// NOTE: it is NV12, so not rgba
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

pub use gstreamer_playbin::GVideoUrl;

pub use gstreamer_pipewire::GVideoPipewire;

#[derive(Debug, Default)]
struct State {
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
/// The container for the gstreamer
/// Current it supports UrlPlayer and Pipewire
/// And also a empty [GVideo::None]
#[derive(Debug)]
pub enum GVideo {
    UrlPlayer(GVideoUrl),
    PipeWire(GVideoPipewire),
    None,
}

impl Default for GVideo {
    fn default() -> Self {
        Self::empty()
    }
}

mod seal {
    use super::*;
    /// The builder to modify a [GVideo], whose inner will be [GVideoUrl]
    #[derive(Debug)]
    pub struct UrlBinBuilderRef<'a> {
        video: &'a mut GVideo,
        url: url::Url,
        is_live: bool,
        file: Option<PathBuf>,
    }

    impl<'a> UrlBinBuilderRef<'a> {
        pub(crate) fn new(video: &'a mut GVideo, url: url::Url, is_live: bool) -> Self {
            Self {
                video,
                url,
                is_live,
                file: None,
            }
        }
        /// save it to a file
        pub fn save_file<P: AsRef<Path>>(mut self, file: P) -> Self {
            self.file = Some(file.as_ref().to_path_buf());
            self
        }

        /// maybe save it to a file
        pub fn save_file_maybe<P: AsRef<Path>>(mut self, file: Option<P>) -> Self {
            self.file = file.map(|f| f.as_ref().to_path_buf());
            self
        }

        /// finish the modification
        pub fn finish(self) -> Result<(), IcedGStreamerError> {
            *self.video = match self.file {
                Some(file) => GVideo::UrlPlayer(GVideoUrl::new_url_and_record(
                    &self.url,
                    self.is_live,
                    file,
                )?),
                None => GVideo::UrlPlayer(GVideoUrl::new_url(&self.url, self.is_live)?),
            };
            Ok(())
        }
    }
    /// The builder to modify a [GVideo], whose inner will be [GVideoPipewire]
    #[derive(Debug)]
    pub struct PipeWireBuilderRef<'a> {
        video: &'a mut GVideo,
        path: u32,
        fd: RawFd,
        file: Option<PathBuf>,
    }

    impl<'a> PipeWireBuilderRef<'a> {
        pub(crate) fn new(video: &'a mut GVideo, path: u32, fd: RawFd) -> Self {
            Self {
                video,
                path,
                fd,
                file: None,
            }
        }
        /// save it to a file
        pub fn save_file<P: AsRef<Path>>(mut self, file: P) -> Self {
            self.file = Some(file.as_ref().to_path_buf());
            self
        }
        /// maybe save it to a file
        pub fn save_file_maybe<P: AsRef<Path>>(mut self, file: Option<P>) -> Self {
            self.file = file.map(|f| f.as_ref().to_path_buf());
            self
        }
        /// finish the modification
        pub fn finish(self) -> Result<(), IcedGStreamerError> {
            *self.video = match self.file {
                Some(file) => GVideo::PipeWire(GVideoPipewire::new_pipewire_and_record(
                    self.path, self.fd, file,
                )?),
                None => GVideo::PipeWire(GVideoPipewire::new_pipewire(self.path, self.fd)?),
            };
            Ok(())
        }
    }

    /// The builder to build a [GVideo], whose inner is [GVideoPipewire]
    #[derive(Debug)]
    pub struct PipeWireBuilder {
        video: GVideo,
        path: u32,
        fd: RawFd,
        file: Option<PathBuf>,
    }

    impl PipeWireBuilder {
        pub(crate) fn new(path: u32, fd: RawFd) -> Self {
            Self {
                video: GVideo::None,
                path,
                fd,
                file: None,
            }
        }
        /// save it to a file
        pub fn save_file<P: AsRef<Path>>(mut self, file: P) -> Self {
            self.file = Some(file.as_ref().to_path_buf());
            self
        }
        /// maybe save it to a file
        pub fn save_file_maybe<P: AsRef<Path>>(mut self, file: Option<P>) -> Self {
            self.file = file.map(|f| f.as_ref().to_path_buf());
            self
        }
        /// build a [GVideo]
        pub fn build(mut self) -> Result<GVideo, IcedGStreamerError> {
            self.video = match self.file {
                Some(file) => GVideo::PipeWire(GVideoPipewire::new_pipewire_and_record(
                    self.path, self.fd, file,
                )?),
                None => GVideo::PipeWire(GVideoPipewire::new_pipewire(self.path, self.fd)?),
            };
            Ok(self.video)
        }
    }

    /// The builder to build a [GVideo], whose inner is [GVideoUrl]
    #[derive(Debug)]
    pub struct UrlBinBuilder {
        video: GVideo,
        url: url::Url,
        is_live: bool,
        file: Option<PathBuf>,
    }

    impl UrlBinBuilder {
        pub(crate) fn new(url: url::Url, is_live: bool) -> Self {
            Self {
                video: GVideo::None,
                url,
                is_live,
                file: None,
            }
        }

        /// save it to a file
        pub fn save_file<P: AsRef<Path>>(mut self, file: P) -> Self {
            self.file = Some(file.as_ref().to_path_buf());
            self
        }

        /// maybe save it to a file
        pub fn save_file_maybe<P: AsRef<Path>>(mut self, file: Option<P>) -> Self {
            self.file = file.map(|f| f.as_ref().to_path_buf());
            self
        }
        /// build a [GVideo]
        pub fn build(mut self) -> Result<GVideo, IcedGStreamerError> {
            self.video = match self.file {
                Some(file) => GVideo::UrlPlayer(GVideoUrl::new_url_and_record(
                    &self.url,
                    self.is_live,
                    file,
                )?),
                None => GVideo::UrlPlayer(GVideoUrl::new_url(&self.url, self.is_live)?),
            };
            Ok(self.video)
        }
    }
}
use seal::*;

impl GVideo {
    /// Open a empty video instance
    pub fn empty() -> Self {
        Self::None
    }

    /// create a new [PipeWireBuilderRef], this is used to rebuild a [GVideo]
    pub fn open_pipewire<'a>(&'a mut self, path: u32, fd: RawFd) -> PipeWireBuilderRef<'a> {
        PipeWireBuilderRef::new(self, path, fd)
    }

    /// create a new [UrlBinBuilderRef], this is used to rebuild a [GVideo]
    pub fn open_url<'a>(&'a mut self, url: url::Url, is_live: bool) -> UrlBinBuilderRef<'a> {
        UrlBinBuilderRef::new(self, url, is_live)
    }

    /// create a new [PipeWireBuilder], this is used to build a [GVideo]
    pub fn new_pipewire(path: u32, fd: RawFd) -> PipeWireBuilder {
        PipeWireBuilder::new(path, fd)
    }

    /// create a new [UrlBinBuilder], this is used to build a [GVideo]
    pub fn new_url(url: url::Url, is_live: bool) -> UrlBinBuilder {
        UrlBinBuilder::new(url, is_live)
    }

    /// cast the file from [GVideo] to [GVideoUrl], if not match, then panic
    pub fn as_url(&self) -> &GVideoUrl {
        let Self::UrlPlayer(url_player) = &self else {
            panic!("Not this type");
        };
        url_player
    }

    /// cast the file from [GVideo] to [GVideoPipewire], if not match, then panic
    pub fn as_pw(&self) -> &GVideoPipewire {
        let Self::PipeWire(pw_instance) = &self else {
            panic!("Not this type");
        };
        pw_instance
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

/// The main container for a gstreamer task
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
    #[error("NO extension")]
    NoExtension,
    #[error("Unsupported extension")]
    UnsupportedExtension,
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
        self.source.send_event(gst::event::Eos::new());
        self.alive.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamType {
    UrlPlayer,
    PipeWire,
    Empty,
}

impl<const X: usize> GVideoInner<X> {
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
