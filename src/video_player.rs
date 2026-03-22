use std::marker::PhantomData;

use crate::GVideo;
use crate::StreamType;
use gstreamer as gst;
use gstreamer::glib;
use gstreamer::prelude::*;
use iced_core::{image, layout, Widget};
use iced_core::{ContentFit, Point, Rectangle, Rotation, Size, Vector};
use std::time::Duration;

pub struct VideoPlayer<'a, const X: usize, Message, Theme = iced_core::Theme> {
    video: &'a GVideo<X>,
    content_fit: iced_core::ContentFit,
    width: iced_core::Length,
    height: iced_core::Length,
    on_end_of_stream: Option<Message>,
    on_error: Option<Box<dyn Fn(&glib::Error) -> Message + 'a>>,
    on_duration_changed: Option<Box<dyn Fn(Duration) -> Message + 'a>>,
    on_position_changed: Option<Box<dyn Fn(Duration) -> Message + 'a>>,
    _theme: PhantomData<Theme>,
    _message: PhantomData<Message>,
}

impl<'a, const X: usize, Message, Theme> VideoPlayer<'a, X, Message, Theme> {
    pub fn new(video: &'a GVideo<X>) -> Self {
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

fn drawing_bounds<Renderer>(
    renderer: &Renderer,
    bounds: Rectangle,
    handle: &image::Handle,
    region: Option<Rectangle<u32>>,
    content_fit: ContentFit,
    rotation: Rotation,
    scale: f32,
) -> Rectangle
where
    Renderer: image::Renderer<Handle = image::Handle>,
{
    let original_size = renderer.measure_image(handle).unwrap_or_default();
    let image_size = crop(original_size, region);
    let rotated_size = rotation.apply(image_size);
    let adjusted_fit = content_fit.fit(rotated_size, bounds.size());

    let fit_scale = Vector::new(
        adjusted_fit.width / rotated_size.width,
        adjusted_fit.height / rotated_size.height,
    );

    let final_size = image_size * fit_scale * scale;

    let (crop_offset, final_size) = if let Some(region) = region {
        let x = region.x.min(original_size.width) as f32;
        let y = region.y.min(original_size.height) as f32;
        let width = image_size.width;
        let height = image_size.height;

        let ratio = Vector::new(
            original_size.width as f32 / width,
            original_size.height as f32 / height,
        );

        let final_size = final_size * ratio;

        let scale = Vector::new(
            final_size.width / original_size.width as f32,
            final_size.height / original_size.height as f32,
        );

        let offset = match content_fit {
            ContentFit::None => Vector::new(x * scale.x, y * scale.y),
            _ => Vector::new(
                ((original_size.width as f32 - width) / 2.0 - x) * scale.x,
                ((original_size.height as f32 - height) / 2.0 - y) * scale.y,
            ),
        };

        (offset, final_size)
    } else {
        (Vector::ZERO, final_size)
    };

    let position = match content_fit {
        ContentFit::None => Point::new(
            bounds.x + (rotated_size.width - adjusted_fit.width) / 2.0,
            bounds.y + (rotated_size.height - adjusted_fit.height) / 2.0,
        ),
        _ => Point::new(
            bounds.center_x() - final_size.width / 2.0,
            bounds.center_y() - final_size.height / 2.0,
        ),
    };

    Rectangle::new(position + crop_offset, final_size)
}

fn crop(size: Size<u32>, region: Option<Rectangle<u32>>) -> Size<f32> {
    if let Some(region) = region {
        Size::new(
            region.width.min(size.width) as f32,
            region.height.min(size.height) as f32,
        )
    } else {
        Size::new(size.width as f32, size.height as f32)
    }
}

/// Draws an [`Image`]
pub fn draw<Renderer>(
    renderer: &mut Renderer,
    layout: iced_core::Layout<'_>,
    handle: &image::Handle,
    crop: Option<iced_core::Rectangle<u32>>,
    border_radius: iced_core::border::Radius,
    content_fit: iced_core::ContentFit,
    filter_method: image::FilterMethod,
    rotation: iced_core::Rotation,
    opacity: f32,
    scale: f32,
) where
    Renderer: image::Renderer<Handle = image::Handle>,
{
    let bounds = layout.bounds();
    let drawing_bounds =
        drawing_bounds(renderer, bounds, handle, crop, content_fit, rotation, scale);

    renderer.draw_image(
        image::Image {
            handle: handle.clone(),
            border_radius,
            filter_method,
            rotation: rotation.radians(),
            opacity,
            snap: true,
        },
        drawing_bounds,
        bounds,
    );
}

impl<const X: usize, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VideoPlayer<'_, X, Message, Theme>
where
    Message: Clone,
    Renderer: image::Renderer<Handle = image::Handle>,
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
        let Ok(state) = self.video.state.read() else {
            return;
        };
        let Some(handle) = state.handle.as_ref() else {
            return;
        };
        draw(
            renderer,
            layout,
            handle,
            None,
            Default::default(),
            self.content_fit,
            Default::default(),
            Rotation::default(),
            1.,
            1.,
        );
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
        let mut state = self.video.state.write().unwrap();
        if self.video.stream_type() == StreamType::UrlPlayer {
            if state.get_duration_attempt && self.video.play_state() == gst::State::Playing {
                loop {
                    self.video
                        .source
                        .state(gst::ClockTime::from_seconds(1))
                        .0
                        .unwrap();

                    if let Some(time) = self.video.source.query_duration::<gst::ClockTime>() {
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
                    if let Some(time) = self.video.source.query_position::<gst::ClockTime>() {
                        state.position = std::time::Duration::from_nanos(time.nseconds());
                        if let Some(on_position_changed) = &self.on_position_changed {
                            shell.publish(on_position_changed(state.position));
                        }
                        break;
                    }
                    self.video
                        .source
                        .state(gst::ClockTime::from_seconds(5))
                        .0
                        .unwrap();
                }
            }
            state.volume = self.video.source.property("volume");
        }
        if self.video.play_state() == gst::State::Playing {
            shell.request_redraw();
        }
        while let Some(msg) = self
            .video
            .bus
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
                    }
                }
                _ => {}
            }
        }
    }
}

impl<'a, const X: usize, Message, Theme, Renderer> From<VideoPlayer<'a, X, Message, Theme>>
    for iced_core::Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: image::Renderer<Handle = image::Handle>,
{
    fn from(video_player: VideoPlayer<'a, X, Message, Theme>) -> Self {
        Self::new(video_player)
    }
}
