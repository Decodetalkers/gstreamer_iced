use std::marker::PhantomData;
use std::sync::atomic::Ordering;

use crate::pipeline::VideoPrimitive;
use crate::GVideo;
use crate::StreamType;
use gstreamer as gst;
use gstreamer::glib;
use gstreamer::prelude::*;
use iced_core::{layout, Widget};
use iced_wgpu::primitive::Renderer as PrimitiveRenderer;
use std::time::Duration;

pub struct VideoPlayer<'a, Message, Theme = iced_core::Theme, Renderer = iced_renderer::Renderer> {
    video: &'a GVideo,
    content_fit: iced_core::ContentFit,
    width: iced_core::Length,
    height: iced_core::Length,
    on_end_of_stream: Option<Message>,
    #[allow(clippy::type_complexity)]
    on_error: Option<Box<dyn Fn(&glib::Error) -> Message + 'a>>,
    on_duration_changed: Option<Box<dyn Fn(Duration) -> Message + 'a>>,
    on_position_changed: Option<Box<dyn Fn(Duration) -> Message + 'a>>,
    _theme: PhantomData<Theme>,
    _message: PhantomData<Message>,
    _renderer: PhantomData<Renderer>,
}

impl<'a, Message, Theme, Renderer> VideoPlayer<'a, Message, Theme, Renderer>
where
    Renderer: PrimitiveRenderer,
{
    pub fn new(video: &'a GVideo) -> Self {
        Self {
            video,
            content_fit: iced_core::ContentFit::default(),
            width: iced_core::Length::Shrink,
            height: iced_core::Length::Shrink,
            on_error: None,
            on_end_of_stream: None,
            on_duration_changed: None,
            on_position_changed: None,
            _theme: PhantomData,
            _message: PhantomData,
            _renderer: PhantomData,
        }
    }

    pub fn width(self, width: impl Into<iced_core::Length>) -> Self {
        Self {
            width: width.into(),
            ..self
        }
    }
    pub fn height(self, height: impl Into<iced_core::Length>) -> Self {
        Self {
            height: height.into(),
            ..self
        }
    }
    pub fn on_error<F>(self, on_error: F) -> Self
    where
        F: 'a + Fn(&glib::Error) -> Message,
    {
        VideoPlayer {
            on_error: Some(Box::new(on_error)),
            ..self
        }
    }
    /// Message to send when the video reaches the end of stream (i.e., the video ends).
    pub fn on_end_of_stream(self, on_end_of_stream: Message) -> Self {
        VideoPlayer {
            on_end_of_stream: Some(on_end_of_stream),
            ..self
        }
    }
    pub fn on_duration_changed<F>(self, on_duration_changed: F) -> Self
    where
        F: 'a + Fn(Duration) -> Message,
    {
        VideoPlayer {
            on_duration_changed: Some(Box::new(on_duration_changed)),
            ..self
        }
    }
    pub fn on_position_changed<F>(self, on_position_changed: F) -> Self
    where
        F: 'a + Fn(Duration) -> Message,
    {
        VideoPlayer {
            on_position_changed: Some(Box::new(on_position_changed)),
            ..self
        }
    }
}
impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VideoPlayer<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: PrimitiveRenderer,
{
    fn size(&self) -> iced_core::Size<iced_core::Length> {
        iced_core::Size {
            width: self.width,
            height: self.height,
        }
    }
    fn layout(
        &mut self,
        _tree: &mut iced_core::widget::Tree,
        _renderer: &Renderer,
        limits: &iced_core::layout::Limits,
    ) -> iced_core::layout::Node {
        let image_size = self
            .video
            .frame_data()
            .map(|data| iced_core::Size {
                width: data.width as f32,
                height: data.height as f32,
            })
            .unwrap_or(limits.min());
        let raw_size = limits.resolve(self.width, self.height, image_size);
        let full_size = self.content_fit.fit(image_size, raw_size);
        let final_size = iced_core::Size {
            width: match self.width {
                iced_core::Length::Shrink => f32::min(raw_size.width, full_size.width),
                _ => raw_size.width,
            },
            height: match self.height {
                iced_core::Length::Shrink => f32::min(raw_size.height, full_size.height),
                _ => raw_size.height,
            },
        };

        layout::Node::new(final_size)
    }
    fn draw(
        &self,
        _tree: &iced_core::widget::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &iced_core::renderer::Style,
        layout: iced_core::Layout<'_>,
        _cursor: iced_core::mouse::Cursor,
        _viewport: &iced_core::Rectangle,
    ) {
        let Some(data) = self.video.frame_data() else {
            return;
        };
        let (width, height) = data.size();
        let image_size = iced_core::Size::new(width as f32, height as f32);
        let bounds = layout.bounds();
        let adjusted_fit = self.content_fit.fit(image_size, bounds.size());
        let scale = iced_core::Vector::new(
            adjusted_fit.width / image_size.width,
            adjusted_fit.height / image_size.height,
        );
        let final_size = image_size * scale;

        let position = match self.content_fit {
            iced_core::ContentFit::None => iced_core::Point::new(
                bounds.x + (image_size.width - adjusted_fit.width) / 2.0,
                bounds.y + (image_size.height - adjusted_fit.height) / 2.0,
            ),
            _ => iced_core::Point::new(
                bounds.center_x() - final_size.width / 2.0,
                bounds.center_y() - final_size.height / 2.0,
            ),
        };

        let drawing_bounds = iced_core::Rectangle::new(position, final_size);

        let upload_frame = self
            .video
            .upload_frame()
            .unwrap()
            .swap(false, Ordering::SeqCst);

        let render = |renderer: &mut Renderer| {
            renderer.draw_primitive(
                drawing_bounds,
                VideoPrimitive::new(
                    *self.video.id().unwrap(),
                    self.video.alive().unwrap().clone(),
                    self.video.frame().unwrap(),
                    upload_frame,
                ),
            );
        };

        if adjusted_fit.width > bounds.width || adjusted_fit.height > bounds.height {
            renderer.with_layer(bounds, render);
        } else {
            render(renderer);
        }
    }
    fn update(
        &mut self,
        _tree: &mut iced_core::widget::Tree,
        event: &iced_core::Event,
        _layout: iced_core::Layout<'_>,
        _cursor: iced_core::mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn iced_core::Clipboard,
        shell: &mut iced_core::Shell<'_, Message>,
        _viewport: &iced_core::Rectangle,
    ) {
        let iced_core::Event::Window(iced_core::window::Event::RedrawRequested(_instant)) = event
        else {
            return;
        };
        if self.video.is_none() {
            return;
        }
        let state_o = self.video.state().unwrap();
        let mut state = state_o.write().unwrap();
        if self.video.stream_type() == StreamType::UrlPlayer {
            if state.get_duration_attempt && self.video.play_state() == gst::State::Playing {
                loop {
                    self.video
                        .source()
                        .unwrap()
                        .state(gst::ClockTime::from_seconds(1))
                        .0
                        .unwrap();

                    if let Some(time) = self
                        .video
                        .source()
                        .unwrap()
                        .query_duration::<gst::ClockTime>()
                    {
                        state.duration = std::time::Duration::from_nanos(time.nseconds());
                        if let Some(on_duration_changed) = &self.on_duration_changed {
                            shell.publish(on_duration_changed(state.duration));
                        }
                        break;
                    }
                }
                state.get_duration_attempt = false;
            }
            if state.duration.as_nanos() != 0 {
                loop {
                    if let Some(time) = self
                        .video
                        .source()
                        .unwrap()
                        .query_position::<gst::ClockTime>()
                    {
                        state.position = std::time::Duration::from_nanos(time.nseconds());
                        if let Some(on_position_changed) = &self.on_position_changed {
                            shell.publish(on_position_changed(state.position));
                        }
                        break;
                    }
                    self.video
                        .source()
                        .unwrap()
                        .state(gst::ClockTime::from_seconds(5))
                        .0
                        .unwrap();
                }
            }
            state.volume = self.video.source().unwrap().property("volume");
        }
        if self.video.play_state() == gst::State::Playing {
            shell.request_redraw();
        }
        while let Some(msg) = self
            .video
            .bus()
            .unwrap()
            .pop_filtered(&[gst::MessageType::Error, gst::MessageType::Eos])
        {
            match msg.view() {
                gst::MessageView::Error(err) => {
                    log::error!("bus returned an error: {err}");
                    if let Some(ref on_error) = self.on_error {
                        shell.publish(on_error(&err.error()))
                    };
                }
                gst::MessageView::Eos(_eos) => {
                    if let Some(on_end_of_stream) = self.on_end_of_stream.clone() {
                        shell.publish(on_end_of_stream);
                        self.video.alive().unwrap().swap(false, Ordering::SeqCst);
                    }
                }
                _ => {}
            }
        }
    }
}

impl<'a, Message, Theme, Renderer> From<VideoPlayer<'a, Message, Theme, Renderer>>
    for iced_core::Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + PrimitiveRenderer,
{
    fn from(video_player: VideoPlayer<'a, Message, Theme, Renderer>) -> Self {
        Self::new(video_player)
    }
}
