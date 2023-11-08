use iced::widget::{image, text, Image};
use iced::{executor, widget::container, Application, Theme};
use iced::{Command, Length, Settings};

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;

#[derive(Debug, Default)]
struct InitFlage {
    url: String,
}

fn main() -> iced::Result {
    GstreamserIced::run(Settings {
        flags: InitFlage {
            url: "https://gstreamer.freedesktop.org/data/media/sintel_trailer-480p.webm"
                .to_string(),
        },
        ..Settings::default()
    })
}

#[derive(Debug)]
struct GstreamserIced {
    url: String,
    //pipeline: gst::Pipeline,
}

#[derive(Debug, Clone, Copy)]
enum GstreamerMessage {}

impl Application for GstreamserIced {
    type Theme = Theme;
    type Flags = InitFlage;
    type Executor = executor::Default;
    type Message = GstreamerMessage;

    fn view(&self) -> iced::Element<Self::Message> {
        container(text("test"))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    fn update(&mut self, _message: Self::Message) -> iced::Command<Self::Message> {
        Command::none()
    }

    fn title(&self) -> String {
        "Test".to_string()
    }

    fn new(flags: Self::Flags) -> (Self, Command<Self::Message>) {
        gst::init().unwrap();
        let source = gst::parse_launch(&format!("playbin uri=\"{}\" video-sink=\"videoconvert ! videoscale ! appsink name=app_sink caps=video/x-raw,format=BGRA,pixel-aspect-ratio=1/1\"", flags.url)).unwrap();
        let source = source.downcast::<gst::Bin>().unwrap();

        let video_sink: gst::Element = source.property("video-sink");
        let pad = video_sink.pads().get(0).cloned().unwrap();
        let pad = pad.dynamic_cast::<gst::GhostPad>().unwrap();
        let bin = pad
            .parent_element()
            .unwrap()
            .downcast::<gst::Bin>()
            .unwrap();
        let app_sink = bin.by_name("app_sink").unwrap();
        let app_sink = app_sink.downcast::<gst_app::AppSink>().unwrap();

        app_sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    println!("sss");
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        source.set_state(gst::State::Playing).unwrap();

        //let bus = source.bus().unwrap();

        (Self { url: flags.url }, Command::none())
    }
}