use std::marker::PhantomData;

use crate::GVideo;
//use gstreamer as gst;
use iced_core::{layout, Widget};
use iced_wgpu::primitive::Renderer as PrimitiveRenderer;

pub struct VideoPlayer<
    'a,
    const X: usize,
    Message,
    Theme = iced_core::Theme,
    Renderer = iced_renderer::Renderer,
> where
    Renderer: PrimitiveRenderer,
{
    video: &'a GVideo<X>,
    content_fit: iced_core::ContentFit,
    width: iced_core::Length,
    height: iced_core::Length,
    _theme: PhantomData<Theme>,
    _render: PhantomData<Renderer>,
    _message: PhantomData<Message>,
}

impl<'a, const X: usize, Message, Theme, Renderer> VideoPlayer<'a, X, Message, Theme, Renderer>
where
    Renderer: PrimitiveRenderer,
{
    pub fn new(video: &'a GVideo<X>) -> Self {
        Self {
            video,
            content_fit: iced_core::ContentFit::default(),
            width: iced_core::Length::Shrink,
            height: iced_core::Length::Shrink,
            _theme: PhantomData,
            _render: PhantomData,
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
}

impl<const X: usize, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VideoPlayer<'_, X, Message, Theme, Renderer>
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
            .unwrap_or(limits.max());
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
        tree: &iced_core::widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &iced_core::renderer::Style,
        layout: iced_core::Layout<'_>,
        cursor: iced_core::mouse::Cursor,
        viewport: &iced_core::Rectangle,
    ) {
        todo!()
    }
    fn update(
        &mut self,
        _tree: &mut iced_core::widget::Tree,
        _event: &iced_core::Event,
        _layout: iced_core::Layout<'_>,
        _cursor: iced_core::mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn iced_core::Clipboard,
        _shell: &mut iced_core::Shell<'_, Message>,
        _viewport: &iced_core::Rectangle,
    ) {
    }
}

impl<'a, const X: usize, Message, Theme, Renderer>
    From<VideoPlayer<'a, X, Message, Theme, Renderer>>
    for iced_core::Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + PrimitiveRenderer,
{
    fn from(video_player: VideoPlayer<'a, X, Message, Theme, Renderer>) -> Self {
        Self::new(video_player)
    }
}
