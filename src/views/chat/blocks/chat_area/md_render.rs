use std::sync::Arc;

use gpui::{
    AbsoluteLength, App, Corners, ElementId, Entity, Font, FontStyle, FontWeight, Hsla, Pixels,
    Rgba, SharedString, Styled, TextRun, Window, px,
};
use gpui_tesserae::{
    ElementIdExt,
    primitives::selectable_layout::{
        ChildClickHandler, DecorationDisplay, InlineStyles, InlinedChild, SelectableLayout,
        SelectableLayoutState,
    },
    theme::ThemeExt,
};
use tree_sitter::{InputEdit, Node, Parser, Point, Tree};

/// Round a pixel value to device-aligned increments for crisp rendering.
fn round_px(window: &Window, value: Pixels) -> Pixels {
    let increment = if window.scale_factor() >= 2.0 {
        0.5
    } else {
        1.0
    };
    let v = value.to_f64() as f32;
    px((v / increment).round() * increment)
}

struct MdSpan {
    text: String,
    font: Font,
    size: Pixels,
    color: Hsla,
    underline: Option<gpui::UnderlineStyle>,
    strikethrough: Option<gpui::StrikethroughStyle>,
    decoration: Option<InlineStyles>,
    click_handler: Option<ChildClickHandler>,
}

impl InlinedChild for MdSpan {
    fn copy_text(&self) -> SharedString {
        SharedString::from(self.text.clone())
    }

    fn text_run(&self, len: usize) -> TextRun {
        TextRun {
            len,
            font: self.font.clone(),
            color: self.color,
            background_color: None,
            underline: self.underline.clone(),
            strikethrough: self.strikethrough.clone(),
        }
    }

    fn font_size(&self) -> Option<Pixels> {
        Some(self.size)
    }

    fn decoration(&self) -> Option<InlineStyles> {
        self.decoration.clone()
    }

    fn on_click(&self) -> Option<ChildClickHandler> {
        self.click_handler.clone()
    }
}

#[derive(Clone)]
struct StyleContext {
    font: Font,
    size: Pixels,
    color: Hsla,
    underline: Option<gpui::UnderlineStyle>,
    strikethrough: Option<gpui::StrikethroughStyle>,
    decoration: Option<InlineStyles>,
}

pub struct MdParseState {
    parser: Parser,
    tree: Option<Tree>,
    last_content_len: usize,
}

impl MdParseState {
    fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_astrum_md::LANGUAGE.into())
            .expect("failed to load astrum_md grammar");
        Self {
            parser,
            tree: None,
            last_content_len: 0,
        }
    }

    fn parse(&mut self, content: &str) -> &Tree {
        if let Some(old_tree) = &mut self.tree {
            if content.len() != self.last_content_len {
                let old_len = self.last_content_len;
                let new_len = content.len();
                old_tree.edit(&InputEdit {
                    start_byte: old_len,
                    old_end_byte: old_len,
                    new_end_byte: new_len,
                    start_position: Point::new(0, 0),
                    old_end_position: Point::new(0, 0),
                    new_end_position: Point::new(0, 0),
                });
                self.tree = self.parser.parse(content, Some(old_tree));
            }
        } else {
            self.tree = self.parser.parse(content, None);
        }
        self.last_content_len = content.len();
        self.tree.as_ref().unwrap()
    }
}

struct HeadingStyle {
    font_size: Pixels,
    font_weight: FontWeight,
}

fn resolve_heading_style(
    kind: &str,
    sizes: &gpui_tesserae::theme::ThemeTextSizes,
    weights: &gpui_tesserae::theme::ThemeTextWeights,
    rem: Pixels,
) -> HeadingStyle {
    let (abs_size, weight) = match kind {
        "heading1" => (sizes.heading_xl, weights.heading_xl),
        "heading2" => (sizes.heading_lg, weights.heading_lg),
        "heading3" => (sizes.heading_md, weights.heading_md),
        "heading4" => (sizes.heading_sm, weights.heading_sm),
        "heading5" => (sizes.body, weights.body),
        "heading6" => (sizes.caption, weights.caption),
        _ => (sizes.heading_sm, weights.body),
    };

    let font_size = match abs_size {
        AbsoluteLength::Pixels(px) => px,
        AbsoluteLength::Rems(rems) => rems.to_pixels(rem),
    };

    HeadingStyle {
        font_size,
        font_weight: FontWeight(weight),
    }
}

pub fn render_markdown(
    content: &str,
    base_id: &ElementId,
    text_color: Rgba,
    selection_color: Hsla,
    bg_color: Hsla,
    window: &mut Window,
    cx: &mut App,
) -> SelectableLayout {
    let theme = cx.get_theme();
    let default_font_family = theme.layout.text.default_font.family[0].clone();
    let mono_font_family = theme.layout.text.mono_font.family[0].clone();
    let default_sizes = theme.layout.text.default_font.sizes.clone();
    let default_weights = theme.layout.text.default_font.weights.clone();
    let mono_sizes = theme.layout.text.mono_font.sizes.clone();
    let mono_weights = theme.layout.text.mono_font.weights.clone();
    let rem = window.rem_size();
    let paragraph_font_size = default_sizes.heading_sm.to_pixels(rem);
    let line_height_def = theme.layout.text.default_font.line_height;
    let caption_font_size = default_sizes.caption.to_pixels(rem);
    let mono_body_size = mono_sizes.body.to_pixels(rem);
    let mono_body_weight = FontWeight(mono_weights.body);
    let mono_caption_size = mono_sizes.caption.to_pixels(rem);
    let mono_caption_weight = FontWeight(mono_weights.caption);
    let secondary_color: Hsla = theme.variants.active(cx).colors.text.secondary.into();
    let link_color: Hsla = theme.variants.active(cx).colors.accent.primary.into();

    let line_height = line_height_def.to_pixels(paragraph_font_size.into(), rem);
    let line_height = round_px(window, line_height);

    let text_color_hsla: Hsla = text_color.into();

    let base_font = Font {
        family: default_font_family.clone(),
        ..Default::default()
    };

    let parse_state: Entity<MdParseState> =
        window.use_keyed_state(base_id.with_suffix("md_parse"), cx, |_window, _cx| {
            MdParseState::new()
        });

    let layout_state: Entity<SelectableLayoutState> =
        window.use_keyed_state(base_id.with_suffix("md_layout"), cx, |_window, cx| {
            SelectableLayoutState::new(cx)
        });

    let mut layout = SelectableLayout::new(
        base_id.with_suffix("md_sel"),
        layout_state,
        base_font.clone(),
        paragraph_font_size,
        line_height,
        text_color_hsla,
    )
    .selection_color(selection_color)
    .selection_rounded(px(6.))
    .selection_rounded_smoothing(1.)
    .max_w_full();

    if content.is_empty() {
        return layout;
    }

    let tree = parse_state.update(cx, |state, _cx| state.parse(content).clone());

    let root = tree.root_node();

    let mut block_idx = 0;
    let mut block_cursor = root.walk();
    for block_node in root.children(&mut block_cursor) {
        let kind = block_node.kind();

        if !block_node.is_named() {
            continue;
        }

        if kind == "blank_line" {
            if block_idx > 0 {
                layout = layout.line_break(1, paragraph_font_size);
            }
            continue;
        }

        if block_idx > 0 {
            layout = layout.line_break(0, paragraph_font_size);
        }

        let is_heading = kind.starts_with("heading");
        let is_subheading = kind == "subheading";

        let (block_font_size, block_font_weight, block_color, block_mono_size, block_mono_weight) =
            if is_heading {
                let hs = resolve_heading_style(kind, &default_sizes, &default_weights, rem);
                let ms = resolve_heading_style(kind, &mono_sizes, &mono_weights, rem);
                (
                    hs.font_size,
                    hs.font_weight,
                    text_color_hsla,
                    ms.font_size,
                    ms.font_weight,
                )
            } else if is_subheading {
                (
                    caption_font_size,
                    FontWeight::NORMAL,
                    secondary_color,
                    mono_caption_size,
                    mono_caption_weight,
                )
            } else {
                (
                    paragraph_font_size,
                    FontWeight::NORMAL,
                    text_color_hsla,
                    mono_body_size,
                    mono_body_weight,
                )
            };

        let block_font = Font {
            family: default_font_family.clone(),
            weight: block_font_weight,
            ..Default::default()
        };

        let base_ctx = StyleContext {
            font: block_font,
            size: block_font_size,
            color: block_color,
            underline: None,
            strikethrough: None,
            decoration: None,
        };

        let spans = walk_inline_nodes(
            block_node,
            content,
            &base_ctx,
            &mono_font_family,
            block_mono_size,
            block_mono_weight,
            bg_color,
            link_color,
        );

        layout = layout.children(spans);
        block_idx += 1;
    }

    layout
}

/// Parse a link token `[text](url)` or `[text](url` (streaming) into (text, url).
fn parse_link_token(raw: &str) -> Option<(&str, &str)> {
    let rest = raw.strip_prefix('[')?;
    let close_bracket = rest.find(']')?;
    let text = &rest[..close_bracket];
    let after = &rest[close_bracket + 1..];
    let url_part = after.strip_prefix('(')?;
    let url = url_part.strip_suffix(')').unwrap_or(url_part);
    Some((text, url))
}

fn walk_inline_nodes(
    node: Node,
    content: &str,
    ctx: &StyleContext,
    mono_font_family: &SharedString,
    mono_font_size: Pixels,
    mono_font_weight: FontWeight,
    bg_color: Hsla,
    link_color: Hsla,
) -> Vec<Box<dyn InlinedChild>> {
    let mut spans: Vec<Box<dyn InlinedChild>> = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "text" | "code_content" | "content" => {
                let text = &content[child.byte_range()];
                if !text.is_empty() {
                    spans.push(Box::new(MdSpan {
                        text: text.to_string(),
                        font: ctx.font.clone(),
                        size: ctx.size,
                        color: ctx.color,
                        underline: ctx.underline.clone(),
                        strikethrough: ctx.strikethrough.clone(),
                        decoration: ctx.decoration.clone(),
                        click_handler: None,
                    }));
                }
            }

            "bold" => {
                let mut styled = ctx.clone();
                styled.font.weight = FontWeight::SEMIBOLD;
                spans.extend(walk_inline_nodes(
                    child, content, &styled, mono_font_family, mono_font_size,
                    mono_font_weight, bg_color, link_color,
                ));
            }

            "italic" => {
                let mut styled = ctx.clone();
                styled.font.style = FontStyle::Italic;
                spans.extend(walk_inline_nodes(
                    child, content, &styled, mono_font_family, mono_font_size,
                    mono_font_weight, bg_color, link_color,
                ));
            }

            "underline" => {
                let mut styled = ctx.clone();
                styled.underline = Some(gpui::UnderlineStyle {
                    thickness: px(1.),
                    color: Some(ctx.color),
                    wavy: false,
                });
                spans.extend(walk_inline_nodes(
                    child, content, &styled, mono_font_family, mono_font_size,
                    mono_font_weight, bg_color, link_color,
                ));
            }

            "strikethrough" => {
                let mut styled = ctx.clone();
                styled.strikethrough = Some(gpui::StrikethroughStyle {
                    thickness: px(1.),
                    color: Some(ctx.color),
                });
                spans.extend(walk_inline_nodes(
                    child, content, &styled, mono_font_family, mono_font_size,
                    mono_font_weight, bg_color, link_color,
                ));
            }

            "code_span" => {
                let mut styled = ctx.clone();
                styled.font.family = mono_font_family.clone();
                styled.font.weight = mono_font_weight;
                styled.size = mono_font_size;
                styled.decoration = Some(
                    InlineStyles::new()
                        .bg(bg_color)
                        .corner_radius(Corners::all(px(6.)))
                        .corner_smoothing(1.)
                        .padding_x(px(4.))
                        .padding_y(px(0.))
                        .display(DecorationDisplay::Block),
                );
                spans.extend(walk_inline_nodes(
                    child, content, &styled, mono_font_family, mono_font_size,
                    mono_font_weight, bg_color, link_color,
                ));
            }

            "link" => {
                let raw = &content[child.byte_range()];
                if let Some((text, url)) = parse_link_token(raw) {
                    if !text.is_empty() {
                        let url = url.to_string();
                        spans.push(Box::new(MdSpan {
                            text: text.to_string(),
                            font: ctx.font.clone(),
                            size: ctx.size,
                            color: link_color,
                            underline: Some(gpui::UnderlineStyle {
                                thickness: px(1.),
                                color: Some(link_color),
                                wavy: false,
                            }),
                            strikethrough: ctx.strikethrough.clone(),
                            decoration: ctx.decoration.clone(),
                            click_handler: Some(Arc::new(move |cx: &mut App| {
                                cx.open_url(&url);
                            })),
                        }));
                    }
                }
            }

            _ => {}
        }
    }

    spans
}
