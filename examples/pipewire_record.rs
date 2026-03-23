use ashpd::desktop::{
    screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType},
    PersistMode,
};
use iced::widget::container;
use iced::widget::{button, column, text};
use iced::Length;
use iced::Task;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::Arc;

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
    iced::application(GProgram::new, GProgram::update, GProgram::view)
        .title(GProgram::title)
        .run()
}

struct GProgram {
    video: GVideo,
    fd: Option<Arc<OwnedFd>>,
    state: gstreamer::State,
}
#[derive(Debug, Clone)]
enum GStreamerIcedMessage {
    Ready((u32, Arc<OwnedFd>)),
    StopRecording,
    StateChanged(gstreamer::State),
}

impl GProgram {
    fn view(&'_ self) -> iced::Element<'_, GStreamerIcedMessage> {
        let btn = button(text("[]")).on_press_maybe(if self.state == PlayingState::Playing {
            Some(GStreamerIcedMessage::StopRecording)
        } else {
            None
        });

        let video = VideoPlayer::new(&self.video)
            .on_state_changed(GStreamerIcedMessage::StateChanged)
            .width(Length::Fill);

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
            GStreamerIcedMessage::StopRecording => {
                self.video.as_pw().stop_record();
                Task::none()
            }
            GStreamerIcedMessage::StateChanged(state) => {
                self.state = state;
                Task::none()
            }
            GStreamerIcedMessage::Ready((path, fd)) => {
                self.fd = Some(fd.clone());
                self.video
                    .open_pipewire_and_record(path, fd.as_raw_fd(), "record.mp4")
                    .unwrap();
                self.state = self.video.play_state();
                Task::none()
            }
        }
    }

    fn title(&self) -> String {
        "Iced Gstreamer".to_string()
    }

    fn new() -> (Self, iced::Task<GStreamerIcedMessage>) {
        let video = GVideo::empty();
        (
            Self {
                fd: None,
                state: video.play_state(),
                video,
            },
            iced::Task::perform(
                async { get_path().await.unwrap() },
                GStreamerIcedMessage::Ready,
            ),
        )
    }
}
