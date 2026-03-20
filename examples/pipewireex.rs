use anyhow::anyhow;
use ashpd::desktop::{
    screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType},
    PersistMode,
};

use iced::widget::container;
use iced::widget::{button, column, image, text, Image};
use iced::Length;
use iced::Task;

static MEDIA_PLAYER: &[u8] = include_bytes!("../resource/popandpipi.jpg");
use gstreamer_iced::*;

async fn get_path() -> anyhow::Result<u32> {
    let proxy = Screencast::new().await?;
    let session = proxy.create_session(Default::default()).await?;
    proxy
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_cursor_mode(CursorMode::Embedded)
                .set_sources(SourceType::Monitor | SourceType::Window | SourceType::Virtual)
                .set_multiple(false)
                .set_restore_token(None)
                .set_persist_mode(PersistMode::DoNot),
        )
        .await?;

    let response = proxy
        .start(&session, None, Default::default())
        .await?
        .response()?;

    for stream in response.streams().iter() {
        println!("node id: {}", stream.pipe_wire_node_id());
        println!("size: {:?}", stream.size());
        println!("position: {:?}", stream.position());
        return Ok(stream.pipe_wire_node_id());
    }
    Err(anyhow!("Not get"))
}
fn main() -> iced::Result {
    iced::application(
        GstreamerIcedProgram::new,
        GstreamerIcedProgram::update,
        GstreamerIcedProgram::view,
    )
    .title(GstreamerIcedProgram::title)
    .subscription(GstreamerIcedProgram::subscription)
    .run()
}

struct GstreamerIcedProgram {
    frame: Option<GstreamerIcedPipewire>,
}
#[derive(Debug, Clone)]
enum GStreamerIcedMessage {
    Gst(GStreamerMessage),
    Ready(u32),
}

impl GstreamerIcedProgram {
    fn view(&'_ self) -> iced::Element<'_, GStreamerIcedMessage> {
        let vframe = match &self.frame {
            Some(frame) => frame,
            None => {
                return container(text("loading"))
                    .center_y(Length::Fill)
                    .center_x(Length::Fill)
                    .into();
            }
        };
        let frame = vframe
            .frame_handle()
            .unwrap_or(image::Handle::from_bytes(MEDIA_PLAYER));

        let btn = match vframe.play_status() {
            PlayStatus::Stop | PlayStatus::End => button(text("|>")).on_press(
                GStreamerIcedMessage::Gst(GStreamerMessage::PlayStatusChanged(PlayStatus::Playing)),
            ),
            PlayStatus::Playing => button(text("[]")).on_press(GStreamerIcedMessage::Gst(
                GStreamerMessage::PlayStatusChanged(PlayStatus::Stop),
            )),
        };
        let video = Image::new(frame).width(Length::Fill);

        container(column![
            video,
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
            GStreamerIcedMessage::Gst(message) => match &mut self.frame {
                Some(frame) => frame.update(message).map(GStreamerIcedMessage::Gst),
                None => Task::none(),
            },
            GStreamerIcedMessage::Ready(path) => {
                self.frame = Some(GstreamerIced::new_pipewire(path).unwrap());
                Task::none()
            }
        }
    }

    fn title(&self) -> String {
        "Iced Gstreamer".to_string()
    }

    fn subscription(&self) -> iced::Subscription<GStreamerIcedMessage> {
        match &self.frame {
            Some(frame) => frame.subscription().map(GStreamerIcedMessage::Gst),
            None => iced::Subscription::none(),
        }
    }

    fn new() -> (Self, iced::Task<GStreamerIcedMessage>) {
        (
            Self { frame: None },
            iced::Task::perform(
                async { get_path().await.unwrap() },
                GStreamerIcedMessage::Ready,
            ),
        )
    }
}
