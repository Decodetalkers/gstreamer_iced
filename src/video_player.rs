use std::marker::PhantomData;
use std::sync::atomic::Ordering;

use crate::GVideo;
use crate::StreamType;
use crate::pipeline::VideoPrimitive;
use gst::State;
use gstreamer as gst;
use gstreamer::glib;
use gstreamer::prelude::*;
use iced_core::{Background, Border, Color, Element, Shadow, Theme, Widget, border, layout};
use iced_wgpu::primitive::Renderer as PrimitiveRenderer;
use std::time::{Duration, Instant};

/// The style of a button.
///
/// If not specified with [`Button::style`]
/// the theme will provide the style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The [`Background`] of the button.
    pub background: Option<Background>,
    /// The text [`Color`] of the button.
    pub text_color: Color,
    /// The [`Border`] of the button.
    pub border: Border,
    /// The [`Shadow`] of the button.
    pub shadow: Shadow,
    /// Whether the button should be snapped to the pixel grid.
    pub snap: bool,
}

impl Style {
    /// Updates the [`Style`] with the given [`Background`].
    pub fn with_background(self, background: impl Into<Background>) -> Self {
        Self {
            background: Some(background.into()),
            ..self
        }
    }
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: None,
            text_color: Color::BLACK,
            border: Border::default(),
            shadow: Shadow::default(),
            snap: false,
        }
    }
}

pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class with the given status.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(primary)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}
fn styled(color: Color) -> Style {
    Style {
        background: Some(Background::Color(color)),
        border: border::rounded(2),
        ..Style::default()
    }
}
/// A primary button; denoting a main action.
pub fn primary(theme: &Theme) -> Style {
    let palette = theme.palette();
    styled(palette.primary)
}

/// VideoPlayer, whose backend is gstreamer
pub struct VideoPlayer<'a, Message, Theme, Renderer = iced_renderer::Renderer>
where
    Theme: Catalog,
{
    video: &'a GVideo,
    content_fit: iced_core::ContentFit,
    width: iced_core::Length,
    height: iced_core::Length,
    on_end_of_stream: Option<Message>,
    #[allow(clippy::type_complexity)]
    on_error: Option<Box<dyn Fn(&glib::Error) -> Message + 'a>>,
    on_duration_changed: Option<Box<dyn Fn(Duration) -> Message + 'a>>,
    on_position_changed: Option<Box<dyn Fn(Duration) -> Message + 'a>>,
    on_state_changed: Option<Box<dyn Fn(State) -> Message + 'a>>,
    status_bar: Option<Element<'a, Message, Theme, Renderer>>,
    status_bar_delay: u64,
    class: Theme::Class<'a>,
    _theme: PhantomData<Theme>,
    _message: PhantomData<Message>,
    _renderer: PhantomData<Renderer>,
}

impl<'a, Message, Theme, Renderer> VideoPlayer<'a, Message, Theme, Renderer>
where
    Renderer: PrimitiveRenderer,
    Theme: Catalog,
{
    /// create a new video player
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
            on_state_changed: None,
            status_bar: None,
            status_bar_delay: 2,
            class: Theme::default(),
            _theme: PhantomData,
            _message: PhantomData,
            _renderer: PhantomData,
        }
    }

    /// set the width of the [VideoPlayer]
    pub fn width(self, width: impl Into<iced_core::Length>) -> Self {
        Self {
            width: width.into(),
            ..self
        }
    }

    /// set the height of the [VideoPlayer]
    pub fn height(self, height: impl Into<iced_core::Length>) -> Self {
        Self {
            height: height.into(),
            ..self
        }
    }

    ///  When gstreamer report an error
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

    /// The duration changed during playing
    pub fn on_duration_changed<F>(self, on_duration_changed: F) -> Self
    where
        F: 'a + Fn(Duration) -> Message,
    {
        VideoPlayer {
            on_duration_changed: Some(Box::new(on_duration_changed)),
            ..self
        }
    }

    /// The play state changed during playing
    pub fn on_state_changed<F>(self, on_state_changed: F) -> Self
    where
        F: 'a + Fn(State) -> Message,
    {
        VideoPlayer {
            on_state_changed: Some(Box::new(on_state_changed)),
            ..self
        }
    }

    /// The position changed during playing
    pub fn on_position_changed<F>(self, on_position_changed: F) -> Self
    where
        F: 'a + Fn(Duration) -> Message,
    {
        VideoPlayer {
            on_position_changed: Some(Box::new(on_position_changed)),
            ..self
        }
    }

    pub fn status_bar(self, status_bar: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        VideoPlayer {
            status_bar: Some(status_bar.into()),
            ..self
        }
    }

    pub fn status_bar_delay(self, status_bar_delay: u64) -> Self {
        VideoPlayer {
            status_bar_delay,
            ..self
        }
    }
}

const HEIGHT: f32 = 40.;

struct VideoState {
    size: Option<iced_core::Size>,
    instant: Instant,
    show: bool,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VideoPlayer<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: PrimitiveRenderer,
    Theme: Catalog,
{
    fn size(&self) -> iced_core::Size<iced_core::Length> {
        iced_core::Size {
            width: self.width,
            height: self.height,
        }
    }

    fn tag(&self) -> iced_core::widget::tree::Tag {
        iced_core::widget::tree::Tag::of::<VideoState>()
    }

    fn state(&self) -> iced_core::widget::tree::State {
        iced_core::widget::tree::State::new(VideoState {
            size: None,
            instant: Instant::now()
                .checked_add(Duration::from_secs(self.status_bar_delay))
                .unwrap(),
            show: false,
        })
    }

    fn layout(
        &mut self,
        tree: &mut iced_core::widget::Tree,
        renderer: &Renderer,
        limits: &iced_core::layout::Limits,
    ) -> iced_core::layout::Node {
        let video_state: &mut VideoState = tree.state.downcast_mut();
        let image_size = video_state.size.unwrap_or(limits.max());
        if video_state.size.is_none() {
            video_state.size = Some(image_size);
        }
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

        let limits = iced_core::layout::Limits::new(
            limits.min(),
            iced_core::Size {
                width: raw_size.width,
                height: HEIGHT,
            },
        );
        let y = final_size.height - HEIGHT;

        match &mut self.status_bar {
            Some(bar) => layout::Node::with_children(
                final_size,
                vec![
                    bar.as_widget_mut()
                        .layout(&mut tree.children[0], renderer, &limits)
                        .move_to((0., y)),
                ],
            ),
            None => layout::Node::new(final_size),
        }
    }

    fn children(&self) -> Vec<iced_core::widget::Tree> {
        match &self.status_bar {
            Some(bar) => vec![iced_core::widget::Tree::new(bar)],
            None => vec![],
        }
    }

    fn diff(&self, tree: &mut iced_core::widget::Tree) {
        if let Some(bar) = &self.status_bar {
            tree.diff_children(&[bar]);
        }
    }

    fn operate<'b>(
        &'b mut self,
        state: &'b mut iced_core::widget::Tree,
        layout: iced_core::Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced_core::widget::Operation<()>,
    ) {
        if let Some(bar) = &mut self.status_bar {
            bar.as_widget_mut().operate(
                &mut state.children[0],
                layout.child(0),
                renderer,
                operation,
            );
        }
    }
    fn draw(
        &self,
        tree: &iced_core::widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &iced_core::renderer::Style,
        layout: iced_core::Layout<'_>,
        cursor: iced_core::mouse::Cursor,
        viewport: &iced_core::Rectangle,
    ) {
        let bounds = layout.bounds();
        let vstyle = theme.style(&self.class);

        renderer.fill_quad(
            iced_core::renderer::Quad {
                bounds,
                border: vstyle.border,
                shadow: vstyle.shadow,
                snap: vstyle.snap,
            },
            Background::Color(Color::BLACK),
        );
        let alive = self.video.alive().unwrap().load(Ordering::Relaxed);
        if !alive {
            return;
        }
        let video_state: &VideoState = tree.state.downcast_ref();
        let Some(image_size) = video_state.size else {
            return;
        };

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

        if video_state.show
            && let Some(status_bar) = &self.status_bar
            && cursor.is_over(*viewport)
        {
            renderer.with_layer(*viewport, |renderer| {
                status_bar.as_widget().draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    layout.child(0),
                    cursor,
                    viewport,
                )
            });
        }
    }
    fn update(
        &mut self,
        tree: &mut iced_core::widget::Tree,
        event: &iced_core::Event,
        layout: iced_core::Layout<'_>,
        cursor: iced_core::mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn iced_core::Clipboard,
        shell: &mut iced_core::Shell<'_, Message>,
        viewport: &iced_core::Rectangle,
    ) {
        let state: &mut VideoState = tree.state.downcast_mut();
        if let Some(status_bar) = &mut self.status_bar {
            status_bar.as_widget_mut().update(
                &mut tree.children[0],
                event,
                layout.child(0),
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }

        let _instant = match event {
            iced_core::Event::Window(iced_core::window::Event::RedrawRequested(instant)) => instant,
            iced_core::Event::Mouse(_) => {
                state.instant = Instant::now()
                    .checked_add(Duration::from_secs(self.status_bar_delay))
                    .unwrap();
                state.show = true;
                return;
            }
            _ => {
                return;
            }
        };
        if state.instant < Instant::now() && state.show {
            state.show = false;
        }
        if self.video.is_none() {
            return;
        }
        if let Some(data) = self.video.frame_data() {
            let (width, height) = data.size();
            let image_size = iced_core::Size::new(width as f32, height as f32);

            state.size = Some(image_size);
        }
        let state_o = self.video.state().unwrap();
        let mut state = state_o.write().unwrap();
        let alive = self.video.alive().unwrap().load(Ordering::Relaxed);
        if self.video.stream_type() == StreamType::UrlPlayer && alive {
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
        shell.request_redraw();

        while let Some(msg) = self.video.bus().unwrap().pop_filtered(&[
            gst::MessageType::Error,
            gst::MessageType::Eos,
            gst::MessageType::StateChanged,
        ]) {
            match msg.view() {
                gst::MessageView::Error(err) => {
                    log::error!("bus returned an error: {err}");
                    if let Some(ref on_error) = self.on_error {
                        shell.publish(on_error(&err.error()))
                    };
                }
                gst::MessageView::Eos(_eos) => {
                    self.video
                        .source()
                        .unwrap()
                        .set_state(gst::State::Null)
                        .unwrap();
                    if let Some(on_state_changed) = &self.on_state_changed {
                        shell.publish(on_state_changed(gst::State::Null));
                    }
                    if let Some(on_end_of_stream) = self.on_end_of_stream.clone() {
                        shell.publish(on_end_of_stream);
                    }
                    self.video.alive().unwrap().swap(false, Ordering::SeqCst);
                }
                gstreamer::MessageView::StateChanged(change) => {
                    if change.current() == gst::State::Playing {
                        self.video.alive().unwrap().swap(true, Ordering::SeqCst);
                    }
                    if let Some(on_state_changed) = &self.on_state_changed {
                        shell.publish(on_state_changed(change.current()));
                    }
                }
                _ => {}
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &iced_core::widget::Tree,
        layout: layout::Layout<'_>,
        cursor: iced_core::mouse::Cursor,
        viewport: &iced_core::Rectangle,
        renderer: &Renderer,
    ) -> iced_core::mouse::Interaction {
        if let Some(status_bar) = &self.status_bar {
            return status_bar.as_widget().mouse_interaction(
                &tree.children[0],
                layout.child(0),
                cursor,
                viewport,
                renderer,
            );
        }
        iced_core::mouse::Interaction::default()
    }
}

impl<'a, Message, Theme, Renderer> From<VideoPlayer<'a, Message, Theme, Renderer>>
    for iced_core::Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + PrimitiveRenderer,
    Theme: Catalog,
{
    fn from(video_player: VideoPlayer<'a, Message, Theme, Renderer>) -> Self {
        Self::new(video_player)
    }
}
