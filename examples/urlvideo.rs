use gstreamer_iced::*;
use iced::widget::container;
use iced::widget::{button, column, row, slider, text};
use iced::{Color, Element, Length};
use std::time::Duration;

fn main() -> iced::Result {
    iced::application(GProgram::new, GProgram::update, GProgram::view)
        .title(GProgram::title)
        .run()
}

#[derive(Debug)]
struct GProgram {
    video: GVideo,
    duration: Duration,
    position: Duration,
    state: gstreamer::State,
}
#[derive(Debug, Clone)]
enum GIcedMessage {
    Jump(f64),
    VolChange(f64),
    RequestStateChange(PlayingState),
    DurationChanged(Duration),
    PositionChanged(Duration),
    StateChanged(gstreamer::State),
}

impl GProgram {
    fn view(&'_ self) -> iced::Element<'_, GIcedMessage> {
        let duration = self.duration.as_secs_f64();
        let pos = self.position.as_secs_f64();

        let btn: Element<GIcedMessage> =
            match self.state {
                PlayingState::Playing => button(text("[]"))
                    .on_press(GIcedMessage::RequestStateChange(PlayingState::Paused)),
                _ => button(text("|>"))
                    .on_press(GIcedMessage::RequestStateChange(PlayingState::Playing)),
            }
            .into();
        let pos_status = text(format!("{:.1} s/{:.1} s", pos, duration)).color(Color::WHITE);
        let du_silder = slider(0.0..=duration, pos, GIcedMessage::Jump);

        let add_vol = button(text("+")).on_press(GIcedMessage::VolChange(0.1));
        let min_vol = button(text("-")).on_press(GIcedMessage::VolChange(-0.1));
        let volcurrent = self.video.as_url().volume() * 100.0;

        let voicetext = text(format!("{:.0} %", volcurrent));

        let duration_component = row![pos_status, du_silder, voicetext, add_vol, min_vol]
            .spacing(2)
            .padding(2)
            .width(Length::Fill);

        let bar = container(column![
            duration_component,
            container(btn).width(Length::Fill).center_x(Length::Fill)
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill);
        VideoPlayer::new(&self.video)
            .on_position_changed(GIcedMessage::PositionChanged)
            .on_duration_changed(GIcedMessage::DurationChanged)
            .on_state_changed(GIcedMessage::StateChanged)
            .status_bar(bar)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn update(&mut self, message: GIcedMessage) -> iced::Task<GIcedMessage> {
        match message {
            GIcedMessage::Jump(step) => {
                self.video
                    .as_url()
                    .seek(std::time::Duration::from_secs(step as u64));
                iced::Task::none()
            }
            GIcedMessage::DurationChanged(duration) => {
                self.duration = duration;
                iced::Task::none()
            }
            GIcedMessage::PositionChanged(position) => {
                self.position = position;
                iced::Task::none()
            }
            GIcedMessage::RequestStateChange(status) => {
                self.video.set_state(status);
                iced::Task::none()
            }
            GIcedMessage::VolChange(vol) => {
                let currentvol = self.video.as_url().volume();
                let newvol = currentvol + vol;
                if newvol >= 0.0 {
                    self.video.as_url().set_volume(newvol);
                }
                iced::Task::none()
            }
            GIcedMessage::StateChanged(state) => {
                self.state = state;
                iced::Task::none()
            }
        }
    }

    fn title(&self) -> String {
        "Iced Gstreamer".to_string()
    }

    fn new() -> Self {
        let url = url::Url::parse(
            "https://gstreamer.freedesktop.org/data/media/sintel_cropped_multilingual.webm",
        )
        .unwrap();
        let video = GVideo::new_url(url, false).build().unwrap();

        Self {
            state: video.play_state(),
            video,
            duration: Default::default(),
            position: Default::default(),
        }
    }
}
