use iced::widget::container;
use iced::widget::{button, column, image, row, slider, text, Image};
use iced::{Element, Length};

use gstreamer_iced::*;

static MEDIA_PLAYER: &[u8] = include_bytes!("../resource/popandpipi.jpg");

fn main() -> iced::Result {
    iced::application(
        GProgram::new,
        GProgram::update,
        GProgram::view,
    )
    .title(GProgram::title)
    .subscription(GProgram::subscription)
    .run()
}

#[derive(Debug)]
struct GProgram {
    frame: GVideoUrl,
}
#[derive(Debug, Clone)]
enum GStreamerIcedMessage {
    Gst(GStreamerMessage),
    Jump(u8),
    VolChange(f64),
}

impl GProgram {
    fn view(&'_ self) -> iced::Element<'_, GStreamerIcedMessage> {
        let frame = self
            .frame
            .frame_handle()
            .unwrap_or(image::Handle::from_bytes(MEDIA_PLAYER));
        let fullduration = self.frame.duration_seconds();
        let current_pos = self.frame.position_seconds();
        let duration = (fullduration / 8.0) as u8;
        let pos = (current_pos / 8.0) as u8;

        let btn: Element<GStreamerIcedMessage> = match self.frame.play_status() {
            PlayStatus::Stop | PlayStatus::End => button(text("|>")).on_press(
                GStreamerIcedMessage::Gst(GStreamerMessage::PlayStatusChanged(PlayStatus::Playing)),
            ),
            PlayStatus::Playing => button(text("[]")).on_press(GStreamerIcedMessage::Gst(
                GStreamerMessage::PlayStatusChanged(PlayStatus::Stop),
            )),
        }
        .into();
        let video = Image::new(frame).width(Length::Fill);

        let pos_status = text(format!("{:.1} s/{:.1} s", current_pos, fullduration));
        let du_silder = slider(0..=duration, pos, GStreamerIcedMessage::Jump);

        let add_vol = button(text("+")).on_press(GStreamerIcedMessage::VolChange(0.1));
        let min_vol = button(text("-")).on_press(GStreamerIcedMessage::VolChange(-0.1));
        let volcurrent = self.frame.volume() * 100.0;

        let voicetext = text(format!("{:.0} %", volcurrent));

        let duration_component = row![pos_status, du_silder, voicetext, add_vol, min_vol]
            .spacing(2)
            .padding(2)
            .width(Length::Fill);

        container(column![
            video,
            duration_component,
            container(btn).width(Length::Fill).center_x(Length::Fill)
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    }

    fn update(&mut self, message: GStreamerIcedMessage) -> iced::Task<GStreamerIcedMessage> {
        match message {
            GStreamerIcedMessage::Gst(message) => {
                self.frame.update(message).map(GStreamerIcedMessage::Gst)
            }
            GStreamerIcedMessage::Jump(step) => {
                self.frame
                    .seek(std::time::Duration::from_secs(step as u64 * 8))
                    .unwrap();
                iced::Task::done(GStreamerIcedMessage::Gst(GStreamerMessage::Update))
            }
            GStreamerIcedMessage::VolChange(vol) => {
                let currentvol = self.frame.volume();
                let newvol = currentvol + vol;
                if newvol >= 0.0 {
                    self.frame.set_volume(newvol);
                }
                iced::Task::done(GStreamerIcedMessage::Gst(GStreamerMessage::Update))
            }
        }
    }

    fn title(&self) -> String {
        "Iced Gstreamer".to_string()
    }

    fn subscription(&self) -> iced::Subscription<GStreamerIcedMessage> {
        self.frame.subscription().map(GStreamerIcedMessage::Gst)
    }

    fn new() -> Self {
        let url = url::Url::parse(
            //"http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/TearsOfSteel.mp4",
            "https://gstreamer.freedesktop.org/data/media/sintel_trailer-480p.webm",
        )
        .unwrap();
        let frame = GVideo::new_url(&url, false).unwrap();

        Self { frame }
    }
}
