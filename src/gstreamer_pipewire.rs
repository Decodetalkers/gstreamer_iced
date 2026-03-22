use super::{FrameData, GVideo, IcedGStreamerError};
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use std::{
    os::fd::RawFd,
    sync::{Arc, RwLock},
};

pub type GVideoPipewire = GVideo<1>;

impl GVideoPipewire {
    /// Accept a pipewire stream, it accept a pipewire path, you may can get it from ashpd, it is
    /// called node.
    pub fn new_pipewire(path: u32, fd: RawFd) -> Result<Self, IcedGStreamerError> {
        gst::init()?;

        let source = gst::Pipeline::new();
        let pipewiresrc = gst::ElementFactory::make("pipewiresrc")
            .property("fd", fd)
            .property("path", path.to_string())
            .build()?;

        let videoconvert = gst::ElementFactory::make("videoconvert").build()?;

        let app_sink_caps = gst::Caps::builder("video/x-raw")
            .field("format", "RGBA")
            .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
            .build();

        let app_sink: gst_app::AppSink = gst_app::AppSink::builder()
            .name("app_sink")
            .caps(&app_sink_caps)
            .build();

        let state = Arc::new(RwLock::new(crate::State::new()));
        let state_c = state.clone();

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
                    state.frame = Some(FrameData {
                        width: width as _,
                        height: height as _,
                        pixels: map.as_slice().to_owned(),
                    });
                    state.handle = state.frame.clone().map(|f| f.into());
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
        })
    }
}
