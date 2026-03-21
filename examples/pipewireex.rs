use ashpd::desktop::{
    screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType},
    PersistMode,
};
use iced::widget::container;
use iced::widget::{button, column, image, text, Image};
use iced::Length;
use iced::Task;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::Arc;

static MEDIA_PLAYER: &[u8] = include_bytes!("../resource/popandpipi.jpg");
use gstreamer_iced::*;

async fn get_path() -> ashpd::Result<(u32, Arc<OwnedFd>)> {
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

    let stream = response
        .streams()
        .first()
        .expect("No stream found or selected")
        .to_owned();
    let path = stream.pipe_wire_node_id();

    let fd = proxy
        .open_pipe_wire_remote(&session, Default::default())
        .await?;

    Ok((path, Arc::new(fd)))
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
    handle: image::Handle,
    fd: Option<Arc<OwnedFd>>,
}
#[derive(Debug, Clone)]
enum GStreamerIcedMessage {
    Gst(GStreamerMessage),
    Ready((u32, Arc<OwnedFd>)),
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

        let btn = match vframe.play_status() {
            PlayStatus::Stop | PlayStatus::End => button(text("|>")).on_press(
                GStreamerIcedMessage::Gst(GStreamerMessage::PlayStatusChanged(PlayStatus::Playing)),
            ),
            PlayStatus::Playing => button(text("[]")).on_press(GStreamerIcedMessage::Gst(
                GStreamerMessage::PlayStatusChanged(PlayStatus::Stop),
            )),
        };
        let video = Image::new(&self.handle).width(Length::Fill);

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
            GStreamerIcedMessage::Gst(GStreamerMessage::Update) => {
                let Some(vframe) = &self.frame else {
                    return Task::none();
                };
                let Some(handle) = vframe.frame_handle() else {
                    return Task::none();
                };
                self.handle = handle;
                Task::none()
            }
            GStreamerIcedMessage::Gst(message) => match &mut self.frame {
                Some(frame) => frame.update(message).map(GStreamerIcedMessage::Gst),
                None => Task::none(),
            },
            GStreamerIcedMessage::Ready((path, fd)) => {
                self.fd = Some(fd.clone());
                self.frame = Some(GstreamerIced::new_pipewire(path, fd.as_raw_fd()).unwrap());
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
            Self {
                frame: None,
                handle: image::Handle::from_bytes(MEDIA_PLAYER),
                fd: None,
            },
            iced::Task::perform(
                async { get_path().await.unwrap() },
                GStreamerIcedMessage::Ready,
            ),
        )
    }
}
