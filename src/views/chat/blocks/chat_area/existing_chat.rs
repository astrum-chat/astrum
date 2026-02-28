use anyml::MessageRole;
use gpui::{
    App, Div, ElementId, Hsla, IntoElement, Overflow, PointRefinement, Rgba, SharedString,
    Stateful, Styled, Window, div, prelude::*, px,
};
use gpui_tesserae::primitives::selectable_text::{SelectableText, SelectableTextState};
use gpui_tesserae::{ElementIdExt, components::ChatBubble, theme::ThemeExt};
use notitia::OrderKey;
use notitia::PrimaryKey;
use std::collections::BTreeMap;

use super::md_render::render_markdown;
use crate::RgbaExt;
use crate::managers::THINK_DELIMITER;
use schema::UniqueId;

pub fn render_existing_chat(
    base_id: &ElementId,
    messages: &BTreeMap<OrderKey, (PrimaryKey<UniqueId>, String, String)>,
) -> Stateful<Div> {
    div()
        .id(base_id.with_suffix("existing_messages"))
        .w_full()
        .h_auto()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(60.))
        .mb(px(-35.))
        .p(px(20.))
        // 20px base padding, 35px to account for margin, 175px is extra.
        .pb(px(20. + 35. + 175.))
        .map(|mut this| {
            this.style().overflow = PointRefinement {
                x: None,
                y: Some(Overflow::Scroll),
            };
            this
        })
        .children(render_messages(messages))
}

fn right_align(child: impl IntoElement) -> Div {
    div()
        .w_full()
        .h_auto()
        .flex()
        .flex_col()
        .justify_end()
        .items_end()
        .child(child)
}

fn render_messages(
    messages: &BTreeMap<OrderKey, (PrimaryKey<UniqueId>, String, String)>,
) -> impl Iterator<Item = ChatMessage> + '_ {
    messages.values().map(|(id, role, content)| {
        ChatMessage::new(id.to_string(), MessageRole::from_str(role), content)
    })
}

enum ContentBlock<'a> {
    Content(&'a str),
    Thinking(&'a str),
}

/// Splits content on `<|think|>` delimiters into alternating content/thinking blocks.
/// Even indices are content, odd indices are thinking. Empty segments are skipped.
fn parse_content_blocks(content: &str) -> Vec<ContentBlock<'_>> {
    if !content.contains(THINK_DELIMITER) {
        return vec![ContentBlock::Content(content)];
    }

    content
        .split(THINK_DELIMITER)
        .enumerate()
        .filter(|(_, segment)| !segment.is_empty())
        .map(|(i, segment)| {
            if i % 2 == 0 {
                ContentBlock::Content(segment)
            } else {
                ContentBlock::Thinking(segment)
            }
        })
        .collect()
}

#[derive(IntoElement)]
struct ChatMessage {
    id: ElementId,
    role: MessageRole,
    content: SharedString,
}

impl ChatMessage {
    fn new(id: impl Into<ElementId>, role: MessageRole, content: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            role,
            content: content.into(),
        }
    }
}

impl RenderOnce for ChatMessage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let active = cx.get_theme().variants.active(cx);
        let text_color = active.colors.text.primary;
        let secondary_text_color: Rgba = active.colors.text.secondary;
        let selection_color: Hsla = active.colors.accent.primary.alpha(0.3).into();

        let is_user = matches!(self.role, MessageRole::User);
        let bg_color: Hsla = if is_user {
            active.colors.background.quaternary
        } else {
            active.colors.background.tertiary
        }
        .into();

        if is_user {
            let md = render_markdown(
                &self.content,
                &self.id,
                text_color,
                selection_color,
                bg_color,
                window,
                cx,
            );
            return right_align(ChatBubble::new("chat_bubble").child(md.max_w_full().w_auto()))
                .into_any_element();
        }

        // Assistant message — parse content blocks for thinking support
        let blocks = parse_content_blocks(&self.content);
        let has_thinking = blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::Thinking(_)));

        if !has_thinking {
            let md = render_markdown(
                &self.content,
                &self.id,
                text_color,
                selection_color,
                bg_color,
                window,
                cx,
            );
            return md.w_full().into_any_element();
        }

        let theme = cx.get_theme();
        let caption_size = theme.layout.text.default_font.sizes.caption;
        let body_size = theme.layout.text.default_font.sizes.body;
        let line_height_def = theme.layout.text.default_font.line_height;
        let rem = window.rem_size();
        let caption_px = caption_size.to_pixels(rem);
        let body_line_height = line_height_def.to_pixels(body_size.into(), rem);

        let mut container = div().w_full().flex().flex_col().gap(body_line_height);

        for (block_idx, block) in blocks.iter().enumerate() {
            match block {
                ContentBlock::Content(text) => {
                    let md = render_markdown(
                        text,
                        &self.id.with_suffix(format!("content_{block_idx}")),
                        text_color,
                        selection_color,
                        bg_color,
                        window,
                        cx,
                    );
                    container = container.child(md.w_full());
                }
                ContentBlock::Thinking(text) => {
                    let label_id = self.id.with_suffix(format!("thinking_label_{block_idx}"));
                    let label_state =
                        window.use_keyed_state(label_id.clone(), cx, |_window, cx| {
                            let mut state = SelectableTextState::new(cx);
                            state.text("Thinking");
                            state
                        });
                    let label = SelectableText::new(label_id, label_state)
                        .text_color(Hsla::from(secondary_text_color))
                        .text_size(caption_px)
                        .selection_color(selection_color)
                        .selection_rounded(px(6.))
                        .selection_rounded_smoothing(1.);

                    let md = render_markdown(
                        text,
                        &self.id.with_suffix(format!("thinking_{block_idx}")),
                        secondary_text_color,
                        selection_color,
                        bg_color,
                        window,
                        cx,
                    );

                    container = container.child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(label)
                            .child(md.w_full()),
                    );
                }
            }
        }

        container.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_thinking_delimiters() {
        let blocks = parse_content_blocks("Hello world");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0], ContentBlock::Content("Hello world")));
    }

    #[test]
    fn test_thinking_then_content() {
        let content = "\n<|think|>\nthinking here\n<|think|>\nresponse here";
        let blocks = parse_content_blocks(content);
        assert_eq!(blocks.len(), 2);
        assert!(matches!(blocks[0], ContentBlock::Thinking("thinking here")));
        assert!(matches!(blocks[1], ContentBlock::Content("response here")));
    }

    #[test]
    fn test_multiple_thinking_blocks() {
        let content = "\n<|think|>\nthink1\n<|think|>\ncontent1\n<|think|>\nthink2\n<|think|>\ncontent2";
        let blocks = parse_content_blocks(content);
        assert_eq!(blocks.len(), 4);
        assert!(matches!(blocks[0], ContentBlock::Thinking("think1")));
        assert!(matches!(blocks[1], ContentBlock::Content("content1")));
        assert!(matches!(blocks[2], ContentBlock::Thinking("think2")));
        assert!(matches!(blocks[3], ContentBlock::Content("content2")));
    }

    #[test]
    fn test_content_first_then_thinking() {
        let content = "content first\n<|think|>\nthen thinking\n<|think|>\nmore content";
        let blocks = parse_content_blocks(content);
        assert_eq!(blocks.len(), 3);
        assert!(matches!(blocks[0], ContentBlock::Content("content first")));
        assert!(matches!(blocks[1], ContentBlock::Thinking("then thinking")));
        assert!(matches!(blocks[2], ContentBlock::Content("more content")));
    }

    #[test]
    fn test_empty_segments_skipped() {
        let content = "\n<|think|>\n\n<|think|>\ncontent";
        let blocks = parse_content_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0], ContentBlock::Content("content")));
    }
}
