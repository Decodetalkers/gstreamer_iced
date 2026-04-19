use super::{FrameData, GVideoInner, IcedGStreamerError};
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use std::{
    os::fd::RawFd,
    path::Path,
    sync::{Arc, Mutex, RwLock, atomic::AtomicBool},
};

/// The main container for a gstreamer task
/// For pipewire
pub type GVideoPipewire = GVideoInner<1>;

impl GVideoPipewire {
    /// Stop recording the file
    pub fn stop_recording(&self) {
        self.source.send_event(gst::event::Eos::new());
    }
    /// Accept a pipewire stream, it accept a pipewire path, you may can get it from ashpd, it is
    /// called node.
    pub(crate) fn new_pipewire(path: u32, fd: RawFd) -> Result<Self, IcedGStreamerError> {
        gst::init()?;

        let source = gst::Pipeline::new();
        let pipewiresrc = gst::ElementFactory::make("pipewiresrc")
            .property("fd", fd)
            .property("path", path.to_string())
            .build()?;

        let videoconvert = gst::ElementFactory::make("videoconvert").build()?;

        let app_sink_caps = gst::Caps::builder("video/x-raw")
            .field("format", "NV12")
            .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
            .build();

        let app_sink: gst_app::AppSink = gst_app::AppSink::builder()
            .name("app_sink")
            .caps(&app_sink_caps)
            .build();

        let state = Arc::new(RwLock::new(crate::State::new()));

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

                    upload_frame_i.store(true, std::sync::atomic::Ordering::SeqCst);
                    let data = FrameData {
                        width: width as _,
                        height: height as _,
                        pixels: map.as_slice().to_owned(),
                    };
                    *frame_i.lock().map_err(|_| gst::FlowError::Eos)? = Some(data);
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        let app_sink: gst::Element = app_sink.clone().into();
        source.add_many([&pipewiresrc, &videoconvert, &app_sink])?;

        gst::Element::link_many([&pipewiresrc, &videoconvert, &app_sink])?;

        source.set_state(gst::State::Playing)?;

        Ok(Self {
            bus: source.bus().unwrap(),
            source: source.into(),
            state,
            upload_frame,
            frame,
            alive: Arc::new(AtomicBool::new(true)),
            id: crate::id::Id::unique(),
            pending_events: RwLock::new(vec![]),
        })
    }

    pub(crate) fn new_pipewire_and_record<P: AsRef<Path>>(
        path: u32,
        fd: RawFd,
        file: P,
    ) -> Result<Self, IcedGStreamerError> {
        gst::init()?;
        let p = file.as_ref();

        let extension = p.extension().ok_or(IcedGStreamerError::NoExtension)?;

        let encoder = if extension == "flv" {
            gst::ElementFactory::make("flvmux").build()?
        } else if extension == "avi" {
            gst::ElementFactory::make("avimux").build()?
        } else if extension == "mp4" {
            gst::ElementFactory::make("mp4mux").build()?
        } else {
            return Err(IcedGStreamerError::UnsupportedExtension);
        };
        let source = gst::Pipeline::new();
        let pipewiresrc = gst::ElementFactory::make("pipewiresrc")
            .property("fd", fd)
            .property("path", path.to_string())
            .build()?;
        let videoconvert = gst::ElementFactory::make("videoconvert")
            .name("videoconvert1")
            .build()
            .unwrap();
        let videoconvert2 = gst::ElementFactory::make("videoconvert")
            .name("videoconvert2")
            .build()
            .unwrap();
        let x264enc = gst::ElementFactory::make("x264enc")
            .property_from_str("tune", "zerolatency")
            .build()?;
        let filesink = gst::ElementFactory::make("filesink")
            .property("location", p.to_str().unwrap())
            .build()?;

        let app_sink_caps = gst::Caps::builder("video/x-raw")
            .field("format", "NV12")
            .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
            .build();

        let app_sink: gst_app::AppSink = gst_app::AppSink::builder()
            .name("app_sink")
            .caps(&app_sink_caps)
            .build();

        let state = Arc::new(RwLock::new(crate::State::new()));

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

                    upload_frame_i.store(true, std::sync::atomic::Ordering::SeqCst);
                    let data = FrameData {
                        width: width as _,
                        height: height as _,
                        pixels: map.as_slice().to_owned(),
                    };
                    *frame_i.lock().map_err(|_| gst::FlowError::Eos)? = Some(data);
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        let app_sink: gst::Element = app_sink.clone().into();
        let queue1 = gst::ElementFactory::make("queue")
            .name("queue1")
            .property("max-size-buffers", 50_u32)
            .property("max-size-bytes", 0_u32)
            .property("max-size-time", 0_u64)
            .build()?;
        let queue2 = gst::ElementFactory::make("queue")
            .name("queue2")
            .property("max-size-buffers", 50_u32)
            .property("max-size-bytes", 0_u32)
            .property("max-size-time", 0_u64)
            .build()?;
        let tee = gst::ElementFactory::make("tee").name("tee").build()?;
        source.add_many([
            &pipewiresrc,
            &tee,
            &queue1,
            &queue2,
            &videoconvert,
            &app_sink,
            &videoconvert2,
            &x264enc,
            &encoder,
            &filesink,
        ])?;
        gst::Element::link_many([&pipewiresrc, &tee])?;
        gst::Element::link_many([&tee, &queue1, &videoconvert, &app_sink])?;
        gst::Element::link_many([&tee, &queue2, &videoconvert2, &x264enc, &encoder, &filesink])?;
        source.set_state(gst::State::Playing)?;

        Ok(Self {
            bus: source.bus().unwrap(),
            source: source.into(),
            state,
            upload_frame,
            frame,
            alive: Arc::new(AtomicBool::new(true)),
            id: crate::id::Id::unique(),
            pending_events: RwLock::new(vec![]),
        })
    }
}
