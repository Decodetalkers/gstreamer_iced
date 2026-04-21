use std::marker::PhantomData;
use std::sync::atomic::Ordering;

use crate::GVideo;
use crate::StreamType;
use crate::pipeline::VideoPrimitive;
use gst::State;
use gstreamer as gst;
use gstreamer::GenericFormattedValue;
use gstreamer::glib;
use gstreamer::prelude::*;
use iced_core::{
    Background, Border, Color, ContentFit, Element, Point, Rectangle, Shadow, Size, Theme, Vector,
    Widget, border, layout, svg,
};
use iced_wgpu::primitive::Renderer as PrimitiveRenderer;
use std::time::{Duration, Instant};

const PLAY_ICON: &[u8] = include_bytes!("../misc/play.svg");
const PAUSE_ICON: &[u8] = include_bytes!("../misc/pause.svg");

/// The style of a button.
///
/// If not specified with [`Button::style`]
/// the theme will provide the style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The [`Background`] of the video
    pub background: Option<Background>,
    /// The text [`Color`] of the button.
    pub text_color: Color,
    /// The [`Border`] of the button.
    pub border: Border,
    /// The [`Shadow`] of the button.
    pub shadow: Shadow,
    /// Whether the button should be snapped to the pixel grid.
    pub snap: bool,

    pub video_background: Color,
}

impl Style {
    /// Updates the [`Style`] with the given [`Background`].
    pub fn with_background(self, background: impl Into<Background>) -> Self {
        Self {
            background: Some(background.into()),
            ..self
        }
    }
    /// Updates the [`Style`] with the given [`Background`].
    pub fn with_video_background(self, video_background: Color) -> Self {
        Self {
            video_background,
            ..self
        }
    }
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: None,
            text_color: Color::WHITE,
            video_background: Color::BLACK,
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

/// A way to set the background color for video
pub fn video_background_primary<'a>(video_background: Color) -> impl Fn(&Theme) -> Style + 'a {
    move |theme| Style {
        video_background,
        ..primary(theme)
    }
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
    status_bar_height: f32,
    class: Theme::Class<'a>,
    play_icon: svg::Handle,
    pause_icon: svg::Handle,
    _theme: PhantomData<Theme>,
    _message: PhantomData<Message>,
    _renderer: PhantomData<Renderer>,
}

impl<'a, Message, Theme, Renderer> VideoPlayer<'a, Message, Theme, Renderer>
where
    Renderer: PrimitiveRenderer + svg::Renderer,
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
            status_bar_height: 70.,
            class: Theme::default(),
            play_icon: svg::Handle::from_memory(PLAY_ICON),
            pause_icon: svg::Handle::from_memory(PAUSE_ICON),
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
    pub fn status_bar_height(self, status_bar_height: f32) -> Self {
        VideoPlayer {
            status_bar_height,
            ..self
        }
    }
    pub fn status_bar_delay(self, status_bar_delay: u64) -> Self {
        VideoPlayer {
            status_bar_delay,
            ..self
        }
    }

    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }
}
impl<'a, Message, Theme, Renderer> VideoPlayer<'a, Message, Theme, Renderer>
where
    Renderer: svg::Renderer,
    Theme: Catalog,
{
    fn draw_icon(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        layout: iced_core::layout::Layout<'_>,
        viewport: &iced_core::Rectangle,
        opacity: f32,
    ) {
        use iced_core::{ContentFit, Point, Rectangle, Size, Vector};
        let Size { width, height } = renderer.measure_svg(&self.play_icon);
        let image_size = Size::new(width as f32, height as f32);

        let bounds = layout.bounds();
        let adjusted_fit = self
            .content_fit
            .fit(image_size, bounds.size() / PLAY_ICON_SCALE);
        let scale = Vector::new(
            adjusted_fit.width / image_size.width,
            adjusted_fit.height / image_size.height,
        );

        let final_size = image_size * scale;

        let position = match self.content_fit {
            ContentFit::None => Point::new(
                bounds.x + (image_size.width - adjusted_fit.width) / 2.0,
                bounds.y + (image_size.height - adjusted_fit.height) / 2.0,
            ),
            _ => Point::new(
                bounds.center_x() - final_size.width / 2.0,
                bounds.center_y() - final_size.height / 2.0,
            ),
        };

        let drawing_bounds = Rectangle::new(position, final_size);

        let style = theme.style(&self.class);

        renderer.with_layer(*viewport, |renderer| {
            renderer.draw_svg(
                svg::Svg {
                    handle: self.play_icon.clone(),
                    color: style.text_color.into(),
                    rotation: iced_core::Radians(0.),
                    opacity,
                },
                drawing_bounds,
                bounds,
            );
            renderer.draw_svg(
                svg::Svg {
                    handle: self.pause_icon.clone(),
                    color: style.text_color.into(),
                    rotation: iced_core::Radians(0.),
                    opacity: 1. - opacity,
                },
                drawing_bounds,
                bounds,
            );
        });
    }
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    Playing,
    Pause,
}

struct VideoState {
    size: Option<iced_core::Size>,
    icon_size: Option<iced_core::Size>,
    instant: Instant,
    status_bar_shown: bool,
    icon_instant: Instant,
    direction: Direction,
    opacity: f32,
}

const PLAY_ICON_SCALE: f32 = 6.0;

impl VideoState {
    #[inline]
    fn skip_opacity_change(&self) -> bool {
        matches!(
            (self.direction, self.opacity),
            (Direction::Pause, 0.) | (Direction::Playing, 1.)
        )
    }
    fn opacity_change(&mut self) {
        if self.skip_opacity_change() {
            return;
        }
        let duration = Instant::now() - self.icon_instant;
        let timestamp = duration.as_secs_f32();

        match self.direction {
            Direction::Playing => {
                self.opacity = (self.opacity + timestamp).min(1.);
            }
            Direction::Pause => {
                self.opacity = (self.opacity - timestamp).max(0.);
            }
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VideoPlayer<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: PrimitiveRenderer + svg::Renderer,
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
        let (opacity, direction) = if self.video.play_state() == gst::State::Playing {
            (0., Direction::Pause)
        } else {
            (1., Direction::Playing)
        };
        iced_core::widget::tree::State::new(VideoState {
            size: None,
            icon_size: None,
            instant: Instant::now()
                .checked_add(Duration::from_secs(self.status_bar_delay))
                .unwrap(),

            status_bar_shown: false,
            icon_instant: Instant::now().checked_add(Duration::from_secs(1)).unwrap(),
            direction,
            opacity,
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
        if video_state.icon_size.is_none() {
            let iced_core::Size { width, height } = renderer.measure_svg(&self.play_icon);
            video_state.icon_size = Some(iced_core::Size::new(width as f32, height as f32));
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
            iced_core::Size {
                width: limits.min().width,
                height: self.status_bar_height,
            },
            iced_core::Size {
                width: raw_size.width,
                height: self.status_bar_height,
            },
        );

        let y = final_size.height - self.status_bar_height;

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
            vstyle.video_background,
        );
        let video_state: &VideoState = tree.state.downcast_ref();
        if video_state.status_bar_shown
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
            self.draw_icon(renderer, theme, layout, viewport, video_state.opacity);
        }

        let alive = self.video.alive().unwrap().load(Ordering::Relaxed);
        if !alive {
            return;
        }

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
        let video_state: &mut VideoState = tree.state.downcast_mut();
        if let Some(status_bar) = &mut self.status_bar
            && video_state.status_bar_shown
        {
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

        video_state.opacity_change();
        let _instant = match event {
            iced_core::Event::Window(iced_core::window::Event::RedrawRequested(instant)) => instant,
            iced_core::Event::Mouse(event) => {
                video_state.instant = Instant::now()
                    .checked_add(Duration::from_secs(self.status_bar_delay))
                    .unwrap();
                video_state.status_bar_shown = true;
                shell.request_redraw();
                if let Some(icon_size) = video_state.icon_size
                    && let iced_core::mouse::Event::ButtonPressed(iced_core::mouse::Button::Left) =
                        event
                {
                    let bounds = layout.bounds();
                    let adjusted_fit = self
                        .content_fit
                        .fit(icon_size, bounds.size() / PLAY_ICON_SCALE);
                    let scale = Vector::new(
                        adjusted_fit.width / icon_size.width,
                        adjusted_fit.height / icon_size.height,
                    );

                    let final_size = icon_size * scale;

                    let position = match self.content_fit {
                        ContentFit::None => Point::new(
                            bounds.x + (icon_size.width - adjusted_fit.width) / 2.0,
                            bounds.y + (icon_size.height - adjusted_fit.height) / 2.0,
                        ),
                        _ => Point::new(
                            bounds.center_x() - final_size.width / 2.0,
                            bounds.center_y() - final_size.height / 2.0,
                        ),
                    };

                    let drawing_bounds = Rectangle::new(position, final_size);
                    if cursor.is_over(drawing_bounds) {
                        if self.video.play_state() == gst::State::Playing {
                            self.video.set_state(gst::State::Paused);
                        } else {
                            self.video.set_state(gst::State::Playing);
                        }
                    }
                }

                return;
            }
            _ => {
                return;
            }
        };
        if video_state.instant < Instant::now() && video_state.status_bar_shown {
            video_state.status_bar_shown = false;
        }
        if self.video.is_none() {
            return;
        }
        if let Some(data) = self.video.frame_data() {
            let (width, height) = data.size();
            let image_size = Size::new(width as f32, height as f32);

            video_state.size = Some(image_size);
        }

        let alive = self.video.alive().unwrap().load(Ordering::Relaxed);

        let state_o = self.video.state().unwrap();
        let mut state = state_o.write().unwrap();

        if self.video.stream_type() == StreamType::UrlPlayer && alive {
            for event in self.video.pending_events() {
                match event {
                    crate::GsEvent::Jump(position) => {
                        let position: GenericFormattedValue = position.into();
                        let _ = self
                            .video
                            .source()
                            .unwrap()
                            .seek_simple(gst::SeekFlags::FLUSH, position);
                    }
                }
            }
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
        if matches!(
            self.video.play_state(),
            gst::State::Playing | gst::State::Ready
        ) {
            shell.request_redraw();
        }

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
                        if change.current() == gst::State::Playing {
                            video_state.direction = Direction::Pause;
                        } else {
                            video_state.direction = Direction::Playing;
                        }
                        video_state.icon_instant = Instant::now();
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
        _viewport: &iced_core::Rectangle,
        renderer: &Renderer,
    ) -> iced_core::mouse::Interaction {
        let video_state: &VideoState = tree.state.downcast_ref();
        if !video_state.status_bar_shown {
            return iced_core::mouse::Interaction::Hidden;
        }
        let bar_viewport = layout.child(0).bounds();
        if let Some(status_bar) = &self.status_bar
            && cursor.is_over(bar_viewport)
        {
            return status_bar.as_widget().mouse_interaction(
                &tree.children[0],
                layout.child(0),
                cursor,
                &bar_viewport,
                renderer,
            );
        }
        if let Some(icon_size) = video_state.icon_size {
            let bounds = layout.bounds();
            let adjusted_fit = self
                .content_fit
                .fit(icon_size, bounds.size() / PLAY_ICON_SCALE);
            let scale = Vector::new(
                adjusted_fit.width / icon_size.width,
                adjusted_fit.height / icon_size.height,
            );

            let final_size = icon_size * scale;

            let position = match self.content_fit {
                ContentFit::None => Point::new(
                    bounds.x + (icon_size.width - adjusted_fit.width) / 2.0,
                    bounds.y + (icon_size.height - adjusted_fit.height) / 2.0,
                ),
                _ => Point::new(
                    bounds.center_x() - final_size.width / 2.0,
                    bounds.center_y() - final_size.height / 2.0,
                ),
            };

            let drawing_bounds = Rectangle::new(position, final_size);
            if cursor.is_over(drawing_bounds) {
                return iced_core::mouse::Interaction::Pointer;
            }
        }
        iced_core::mouse::Interaction::default()
    }
}

impl<'a, Message, Theme, Renderer> From<VideoPlayer<'a, Message, Theme, Renderer>>
    for iced_core::Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + PrimitiveRenderer + svg::Renderer,
    Theme: Catalog,
{
    fn from(video_player: VideoPlayer<'a, Message, Theme, Renderer>) -> Self {
        Self::new(video_player)
    }
}
