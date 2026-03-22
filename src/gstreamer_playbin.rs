use gst::prelude::*;
use gst::GenericFormattedValue;
use gstreamer as gst;
use gstreamer_app as gst_app;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

use super::{FrameData, GVideoInner, IcedGStreamerError, Position};

pub type GVideoUrl = GVideoInner<0>;

impl GVideoUrl {
    pub fn seek<T>(&self, position: T) -> Result<(), IcedGStreamerError>
    where
        T: Into<Position>,
    {
        let pos: Position = position.into();
        let position: GenericFormattedValue = pos.into();
        self.source.seek_simple(gst::SeekFlags::FLUSH, position)?;

        Ok(())
    }

    /// accept url like from local or from http
    pub fn new_url(url: &url::Url, islive: bool) -> Result<Self, IcedGStreamerError> {
        gst::init()?;

        let video_sink = gst::Bin::new();
        let videoscale = gst::ElementFactory::make("videoscale").build()?;
        let videoconvert = gst::ElementFactory::make("videoconvert").build()?;

        let app_sink_caps = gst::Caps::builder("video/x-raw")
            .field("format", "NV12")
            .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
            .build();

        let app_sink: gst_app::AppSink = gst_app::AppSink::builder()
            .name("my_sink")
            .caps(&app_sink_caps)
            .build();

        let state = Arc::new(RwLock::new(
            crate::State::new().with_try_get_duration(!islive),
        ));
        let state_c = state.clone();

        let upload_frame = Arc::new(AtomicBool::new(false));
        let upload_frame_i = upload_frame.clone();
        let frame = Arc::new(Mutex::new(None));
        let frame_i = frame.clone();
        app_sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;

                    let caps = sample.caps().ok_or(gst::FlowError::Error)?;
                    let s = caps.structure(0).ok_or(gst::FlowError::Error)?;
                    let width = s.get::<i32>("width").map_err(|_| gst::FlowError::Error)?;
                    let height = s.get::<i32>("height").map_err(|_| gst::FlowError::Error)?;
                    let mut state = state_c.write().map_err(|_| gst::FlowError::Error)?;

                    upload_frame_i.store(true, std::sync::atomic::Ordering::SeqCst);
                    let data = FrameData {
                        width: width as _,
                        height: height as _,
                        pixels: map.as_slice().to_owned(),
                    };
                    state.handle = Some(data.clone().into());
                    *frame_i.lock().map_err(|_| gst::FlowError::Eos)? = Some(data);

                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        let app_sink: gst::Element = app_sink.into();

        video_sink.add_many([&videoscale, &videoconvert, &app_sink])?;
        gst::Element::link_many([&videoscale, &videoconvert, &app_sink])?;

        let staticpad = videoscale.static_pad("sink").unwrap();
        let sinkgost = gst::GhostPad::builder_with_target(&staticpad)?.build();
        sinkgost.set_active(true)?;
        video_sink.add_pad(&sinkgost)?;

        let videosource = gst::ElementFactory::make("playbin")
            .property("uri", url.as_str())
            .property("video-sink", video_sink.to_value())
            .build()?;

        let source = videosource.downcast::<gst::Bin>().unwrap();

        Ok(Self {
            bus: source.bus().unwrap(),
            source,
            state,
            upload_frame,
            frame,
            alive: Arc::new(AtomicBool::new(true)),
            id: crate::id::Id::unique(),
        })
    }

    /// get the volume of the video
    pub fn volume(&self) -> f64 {
        let state = self.state.read().unwrap();
        state.volume
    }

    /// only can be set when source is video
    pub fn set_volume(&self, volume: f64) {
        self.source.set_property("volume", volume);
    }

    /// get the duration, if is live or pipewire, it is 0
    pub fn duration(&self) -> std::time::Duration {
        let state = self.state.read().unwrap();
        state.duration
    }

    /// where the video is now
    pub fn position(&self) -> std::time::Duration {
        let state = self.state.read().unwrap();
        state.position
    }

    /// turn duration to seconds
    pub fn duration_seconds(&self) -> f64 {
        let state = self.state.read().unwrap();
        state.duration.as_secs_f64()
    }

    /// turn position to seconds
    pub fn position_seconds(&self) -> f64 {
        let state = self.state.read().unwrap();
        state.position.as_secs_f64()
    }

    /// turn duration to nanos
    pub fn duration_nanos(&self) -> f64 {
        let state = self.state.read().unwrap();
        state.duration.as_secs_f64()
    }

    /// turn position to nanos
    pub fn position_nanos(&self) -> u128 {
        let state = self.state.read().unwrap();
        state.position.as_nanos()
    }
}
