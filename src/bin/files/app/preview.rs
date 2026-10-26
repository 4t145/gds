//! Optional right-hand preview of the single selected entry.
//!
//! The panel carries the entry's name, shared metadata and a bounded,
//! header-only probe of its content. It deliberately shows no location bar:
//! the file's own path is not repeated here. A symlink's target is different
//! information and is shown as its own row.

mod header;
mod info;

use std::{path::Path, sync::Arc};

use gds::i18n;
use gds::time::Timestamp;
use gds::ui::TOOLTIP_DELAY;
use gds_uim::el;
use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Icon};
use gpui_kit::prelude::*;
use gpui_kit::*;

use super::{Browser, table::EntryPresentation};
use crate::directory::{ContentCategory, Entry, EntryKind};
use header::{
    AudioFacts, DirectoryFacts, FileFacts, ImageFacts, Probe, ProbeHint, TextFacts, VideoFacts,
};
use info::LineEndingLabel;

const IMAGE_MAX_BYTES: u64 = 32 * 1024 * 1024;
const MEDIA_HEIGHT: f32 = 168.;
const PANEL_GAP: f32 = 10.;
const PANEL_PADDING: f32 = 16.;
const META_LABEL_WIDTH: f32 = 76.;
const MEDIA_RADIUS: f32 = 8.;
const ICON_SIZE: f32 = 30.;
const SVG_SNIFF_BYTES: usize = 4 * 1024;
/// Values longer than this get a tooltip so a truncated row stays readable.
const LONG_VALUE_CHARS: usize = 28;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Empty,
    Multiple(usize),
    Item(Item),
}

#[derive(Clone, Debug)]
pub(super) struct Item {
    entry: Entry,
}

impl Item {
    fn from_entry(entry: &Entry) -> Self {
        Self {
            entry: entry.clone(),
        }
    }

    /// Whether the entry's bytes and timestamps are unchanged, so the previous
    /// probe and thumbnail can stay. Permission or ownership changes keep them.
    fn same_source(&self, other: &Self) -> bool {
        self.entry.path == other.entry.path
            && self.entry.kind == other.entry.kind
            && self.entry.size == other.entry.size
            && self.entry.modified == other.entry.modified
    }
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.same_source(other) && self.entry.details == other.entry.details
    }
}

impl Eq for Item {}

enum Thumbnail {
    None,
    Loading,
    TooLarge,
    Failed,
    Ready(Arc<Image>),
}

/// Header facts, loaded on the background executor.
enum Facts {
    None,
    Loading,
    Ready(Probe),
}

/// What one background read produced.
enum LoadOutcome {
    File {
        probe: Probe,
        bytes: Vec<u8>,
        format: Option<ImageFormat>,
    },
    Probe(Probe),
    Failed(String),
}

pub(super) struct View {
    target: Target,
    thumbnail: Thumbnail,
    facts: Facts,
    generation: u64,
}

impl View {
    pub(super) fn new() -> Self {
        Self {
            target: Target::Empty,
            thumbnail: Thumbnail::None,
            facts: Facts::None,
            generation: 0,
        }
    }

    /// Point the preview at a new selection. Reading happens on the background
    /// executor; stale completions are ignored by generation.
    pub(super) fn sync(&mut self, target: Target, cx: &mut Context<Self>) {
        if self.target == target {
            return;
        }
        let same_source = matches!(
            (&self.target, &target),
            (Target::Item(previous), Target::Item(next)) if previous.same_source(next)
        );
        self.target = target;
        if same_source {
            cx.notify();
            return;
        }
        self.generation += 1;
        self.replace_thumbnail(Thumbnail::None, cx);
        self.facts = Facts::None;
        self.load(cx);
        cx.notify();
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let Target::Item(item) = &self.target else {
            return;
        };
        let plan = thumbnail_plan(item);
        let wants_thumbnail = plan.keep_bytes;
        self.thumbnail = plan.thumbnail;
        self.facts = Facts::Loading;
        let generation = self.generation;
        let path = item.entry.path.clone();
        let kind = item.entry.kind;
        let size = item.entry.size;
        let hint = probe_hint(&item.entry);
        let display_path = path.display().to_string();
        let budget = if wants_thumbnail {
            IMAGE_MAX_BYTES as usize
        } else {
            header::PREFIX_BYTES
        };
        let work = cx.background_executor().spawn(async move {
            match kind {
                EntryKind::Directory => LoadOutcome::Probe(header::probe_directory(&path)),
                EntryKind::SymbolicLink => match header::probe_symlink(&path) {
                    Some(probe) => LoadOutcome::Probe(probe),
                    None => LoadOutcome::Failed(format!(
                        "Could not read the link target of {display_path}"
                    )),
                },
                _ => match header::read_prefix(&path, budget) {
                    Ok(bytes) => {
                        let sample = &bytes[..bytes.len().min(header::PREFIX_BYTES)];
                        let probe = header::probe_file(size, hint, sample);
                        let format = if wants_thumbnail {
                            format_for(&path, &bytes)
                        } else {
                            None
                        };
                        LoadOutcome::File {
                            probe,
                            bytes,
                            format,
                        }
                    }
                    Err(error) => LoadOutcome::Failed(format!(
                        "Header read failed for {display_path}: {error}"
                    )),
                },
            }
        });
        cx.spawn(async move |this, cx| {
            let outcome = work.await;
            if let Err(error) = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                match outcome {
                    LoadOutcome::File {
                        probe,
                        bytes,
                        format,
                    } => {
                        this.facts = Facts::Ready(probe);
                        if wants_thumbnail {
                            this.apply_thumbnail(format, bytes, cx);
                        }
                    }
                    LoadOutcome::Probe(probe) => this.facts = Facts::Ready(probe),
                    LoadOutcome::Failed(message) => {
                        eprintln!("{message}");
                        this.facts = Facts::None;
                        if wants_thumbnail {
                            this.replace_thumbnail(Thumbnail::Failed, cx);
                        }
                    }
                }
                cx.notify();
            }) {
                eprintln!("Preview update arrived after the view closed: {error}");
            }
        })
        .detach();
    }

    fn apply_thumbnail(&mut self, format: Option<ImageFormat>, bytes: Vec<u8>, cx: &mut App) {
        match format.filter(|_| !bytes.is_empty()) {
            Some(format) => self.replace_thumbnail(
                Thumbnail::Ready(Arc::new(Image::from_bytes(format, bytes))),
                cx,
            ),
            None => self.replace_thumbnail(Thumbnail::Failed, cx),
        }
    }

    pub(super) fn clear(&mut self, cx: &mut Context<Self>) {
        self.target = Target::Empty;
        self.generation += 1;
        self.facts = Facts::None;
        self.replace_thumbnail(Thumbnail::None, cx);
        cx.notify();
    }

    fn replace_thumbnail(&mut self, next: Thumbnail, cx: &mut App) {
        if let Thumbnail::Ready(previous) = std::mem::replace(&mut self.thumbnail, next) {
            previous.remove_asset(cx);
        }
    }

    fn placeholder(&self, icon: IconName, message: SharedString, cx: &App) -> AnyElement {
        el! {
            .div { flex_1; min_h_0; w_full; flex; flex_col; items_center; justify_center; gap: PANEL_GAP.px;
                text_color: $cx.theme().muted_foreground; text_xs; text_center;
                px: PANEL_PADDING.px; py: PANEL_PADDING.px; overflow_hidden;
            } [
                icon.Icon::new { size: ICON_SIZE.px; flex_shrink_0 },
                .div { w_full; min_w_0; text_center } [message],
            ]
        }
        .into_any_element()
    }

    /// The media area, when this target has something to report.
    ///
    /// Files with no available preview (directories, unsupported formats,
    /// unreadable size) get no frame at all: the panel goes straight to the
    /// name and metadata instead of leaving an empty box with a placeholder.
    fn media(&self, cx: &App) -> Option<AnyElement> {
        let fallback = {
            let color = cx.theme().muted_foreground;
            let message = i18n::text("preview-failed", cx);
            move || {
                el! {
                    .div { size_full; flex; flex_col; items_center; justify_center; gap: PANEL_GAP.px;
                        text_color: color; text_xs; text_center;
                    } [
                        IconName::ImageOff.Icon::new { size: ICON_SIZE.px },
                        $(message.clone()),
                    ]
                }
                .into_any_element()
            }
        };
        let loading = {
            let color = cx.theme().muted_foreground;
            let message = i18n::text("preview-loading", cx);
            move || {
                el! {
                    .div { size_full; flex; items_center; justify_center;
                        text_color: color; text_xs;
                    } [$(message.clone())]
                }
                .into_any_element()
            }
        };
        let content = match &self.thumbnail {
            Thumbnail::Ready(image) => {
                let image = image.clone();
                el! {
                    $(img(image)) {
                        object_fit: ObjectFit::Contain; size_full;
                        with_loading: loading; with_fallback: fallback;
                    }
                }
                .into_any_element()
            }
            Thumbnail::Loading => {
                self.placeholder(IconName::Image, i18n::text("preview-loading", cx), cx)
            }
            Thumbnail::TooLarge => {
                self.placeholder(IconName::ImageOff, i18n::text("preview-too-large", cx), cx)
            }
            Thumbnail::Failed => {
                self.placeholder(IconName::ImageOff, i18n::text("preview-failed", cx), cx)
            }
            Thumbnail::None => return None,
        };
        Some(
            el! {
                .div { h: MEDIA_HEIGHT.px; w_full; flex_shrink_0; flex; flex_col;
                    rounded: MEDIA_RADIUS.px;
                    border_1; border_color: $cx.theme().border; bg: $cx.theme().secondary;
                    overflow_hidden;
                } [content]
            }
            .into_any_element(),
        )
    }

    fn item(&self, item: &Item, cx: &App) -> AnyElement {
        let name: SharedString = item.entry.display_name.clone().into();
        let name_hint = text_hint(name.clone());
        let mut rows = self.base_rows(item, cx);
        if let Facts::Ready(probe) = &self.facts {
            rows.extend(self.probe_rows(probe, cx));
        }
        el! {
            .div { flex; flex_col; flex_1; min_h_0; min_w_0; w_full; overflow_hidden } [
                .div {
                    # "details-body"; flex_1; min_h_0; w_full; overflow_y_scroll;
                    flex; flex_col; p: PANEL_PADDING.px; gap: PANEL_PADDING.px;
                } [
                    @if (let Some(media) = self.media(cx))
                        $(media),
                    @end,
                    .div { # "preview-name"; flex_shrink_0; w_full; min_w_0; text_sm; font_weight: FontWeight::SEMIBOLD; truncate; tooltip: name_hint } [name],
                    .div { flex_shrink_0; flex; flex_col; w_full; min_w_0; gap: MEDIA_RADIUS.px } [..rows],
                ],
            ]
        }
        .into_any_element()
    }

    /// Name-adjacent metadata shared by every entry kind.
    fn base_rows(&self, item: &Item, cx: &App) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        let kind = EntryPresentation::type_label(&item.entry, cx);
        rows.push(meta_row(i18n::text("type", cx), kind, cx));
        if let Some(format) = self.format_label(item) {
            rows.push(meta_row(i18n::text("details-format", cx), format, cx));
        }
        if item.entry.kind == EntryKind::File {
            let size: SharedString = EntryPresentation::size(&item.entry).into();
            rows.push(meta_row(i18n::text("size", cx), size, cx));
        }
        if item.entry.details.owner.is_some() {
            let owner = EntryPresentation::owner(&item.entry, cx).text;
            rows.push(meta_row(i18n::text("owner", cx), owner, cx));
        }
        if let Some(group) = item.entry.details.group.as_ref() {
            let group: SharedString = group.label().into_owned().into();
            rows.push(meta_row(i18n::text("group", cx), group, cx));
        }
        if let Some(permissions) = item.entry.details.permissions.as_ref() {
            let value: SharedString = info::permissions(permissions).into();
            rows.push(meta_row(i18n::text("permissions", cx), value, cx));
        }
        rows.push(meta_timestamp(
            i18n::text("modified", cx),
            Timestamp::new("details-modified", item.entry.modified),
            cx,
        ));
        rows.push(meta_timestamp(
            i18n::text("accessed", cx),
            Timestamp::new("details-accessed", item.entry.details.accessed),
            cx,
        ));
        if let Some(created) = item.entry.details.created {
            rows.push(meta_timestamp(
                i18n::text("details-created", cx),
                Timestamp::new("details-created", Some(created)),
                cx,
            ));
        }
        rows
    }

    /// Concrete format from the probe, falling back to the listing's content.
    fn format_label(&self, item: &Item) -> Option<SharedString> {
        if let Facts::Ready(Probe::File(FileFacts::Image(facts))) = &self.facts {
            return Some(facts.format.label().into());
        }
        if let Facts::Ready(Probe::File(FileFacts::Audio(facts))) = &self.facts {
            return Some(facts.format.label().into());
        }
        if let Facts::Ready(Probe::File(FileFacts::Video(facts))) = &self.facts {
            return Some(facts.format.label().into());
        }
        item.entry
            .details
            .display_content()
            .and_then(|content| content.label())
            .map(SharedString::from)
    }

    fn probe_rows(&self, probe: &Probe, cx: &App) -> Vec<AnyElement> {
        match probe {
            Probe::File(FileFacts::Image(facts)) => image_rows(facts, cx),
            Probe::File(FileFacts::Audio(facts)) => audio_rows(facts, cx),
            Probe::File(FileFacts::Video(facts)) => video_rows(facts, cx),
            Probe::File(FileFacts::Text(facts)) => text_rows(facts, cx),
            Probe::File(FileFacts::Unknown) => Vec::new(),
            Probe::Directory(facts) => directory_rows(facts, cx),
            Probe::Symlink(facts) => vec![meta_row(
                i18n::text("details-target", cx),
                facts.target.display().to_string().into(),
                cx,
            )],
        }
    }
}

fn image_rows(facts: &ImageFacts, cx: &App) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    if let Some(size) = facts.size {
        rows.push(meta_row(
            i18n::text("details-dimensions", cx),
            info::dimensions(size).into(),
            cx,
        ));
        rows.push(meta_row(
            i18n::text("details-aspect", cx),
            info::aspect_ratio(size).into(),
            cx,
        ));
    }
    if let Some(bit_depth) = facts.bit_depth.filter(|depth| *depth > 0) {
        rows.push(meta_row(
            i18n::text("details-bit-depth", cx),
            bit_depth.to_string().into(),
            cx,
        ));
    }
    if let Some(alpha) = facts.alpha {
        let value = i18n::text(if alpha { "details-yes" } else { "details-no" }, cx);
        rows.push(meta_row(i18n::text("details-alpha", cx), value, cx));
    }
    rows
}

fn audio_rows(facts: &AudioFacts, cx: &App) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    if let Some(duration) = facts.duration {
        rows.push(meta_row(
            i18n::text("details-duration", cx),
            info::duration(duration).into(),
            cx,
        ));
    }
    if let Some(bitrate) = facts.bitrate.filter(|bitrate| *bitrate > 0) {
        rows.push(meta_row(
            i18n::text("details-bitrate", cx),
            info::bitrate(bitrate).into(),
            cx,
        ));
    }
    if let Some(rate) = facts.sample_rate.filter(|rate| *rate > 0) {
        rows.push(meta_row(
            i18n::text("details-sample-rate", cx),
            info::sample_rate(rate).into(),
            cx,
        ));
    }
    if let Some(channels) = facts.channels.filter(|channels| *channels > 0) {
        rows.push(meta_row(
            i18n::text("details-channels", cx),
            channels.to_string().into(),
            cx,
        ));
    }
    if let Some(bit_depth) = facts.bit_depth.filter(|depth| *depth > 0) {
        rows.push(meta_row(
            i18n::text("details-bit-depth", cx),
            bit_depth.to_string().into(),
            cx,
        ));
    }
    rows
}

fn video_rows(facts: &VideoFacts, cx: &App) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    if let Some(duration) = facts.duration {
        rows.push(meta_row(
            i18n::text("details-duration", cx),
            info::duration(duration).into(),
            cx,
        ));
    }
    if let Some(size) = facts.dimensions {
        rows.push(meta_row(
            i18n::text("details-dimensions", cx),
            info::dimensions(size).into(),
            cx,
        ));
    }
    if let Some(codec) = facts.codec.as_ref() {
        rows.push(meta_row(
            i18n::text("details-codec", cx),
            codec.as_str().into(),
            cx,
        ));
    }
    rows
}

fn text_rows(facts: &TextFacts, cx: &App) -> Vec<AnyElement> {
    let mut rows = vec![meta_row(
        i18n::text("details-encoding", cx),
        facts.encoding.label().into(),
        cx,
    )];
    if let Some(line_endings) = facts.line_endings {
        let value = match info::line_endings(line_endings) {
            LineEndingLabel::Literal(text) => SharedString::from(text),
            LineEndingLabel::Message(id) => i18n::text(id, cx),
        };
        rows.push(meta_row(i18n::text("details-line-endings", cx), value, cx));
    }
    rows
}

fn directory_rows(facts: &DirectoryFacts, cx: &App) -> Vec<AnyElement> {
    let mut rows = vec![
        meta_row(
            i18n::text("details-folders", cx),
            info::count(facts.folders, facts.capped).into(),
            cx,
        ),
        meta_row(
            i18n::text("details-files", cx),
            info::count(facts.files, facts.capped).into(),
            cx,
        ),
    ];
    if facts.links > 0 {
        rows.push(meta_row(
            i18n::text("details-links", cx),
            info::count(facts.links, facts.capped).into(),
            cx,
        ));
    }
    rows
}

impl Render for View {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match &self.target {
            Target::Empty => self.placeholder(IconName::Info, i18n::text("preview-empty", cx), cx),
            Target::Multiple(count) => self.placeholder(
                IconName::Files,
                i18n::count("preview-multiple", *count, cx),
                cx,
            ),
            Target::Item(item) => self.item(item, cx),
        };
        el! {
            .div { flex_1; min_h_0; min_w_0; w_full; flex; flex_col; overflow_hidden } [$(content)]
        }
    }
}

/// Content-category hints for the weak signatures inside the probe.
fn probe_hint(entry: &Entry) -> ProbeHint {
    match entry
        .details
        .display_content()
        .map(|content| content.category)
    {
        Some(ContentCategory::Text | ContentCategory::Code | ContentCategory::Document) => {
            ProbeHint {
                text: true,
                audio: false,
            }
        }
        Some(ContentCategory::Audio) => ProbeHint {
            text: false,
            audio: true,
        },
        _ => ProbeHint::default(),
    }
}

struct ThumbnailPlan {
    thumbnail: Thumbnail,
    keep_bytes: bool,
}

/// Thumbnail state before the read starts, and whether it should keep bytes.
fn thumbnail_plan(item: &Item) -> ThumbnailPlan {
    let none = ThumbnailPlan {
        thumbnail: Thumbnail::None,
        keep_bytes: false,
    };
    if item.entry.kind != EntryKind::File || format_from_extension(&item.entry.path).is_none() {
        return none;
    }
    match item.entry.size {
        Some(size) if size <= IMAGE_MAX_BYTES => ThumbnailPlan {
            thumbnail: Thumbnail::Loading,
            keep_bytes: true,
        },
        Some(_) => ThumbnailPlan {
            thumbnail: Thumbnail::TooLarge,
            keep_bytes: false,
        },
        None => none,
    }
}

/// The format GPUI should decode these bytes as, preferring the content over
/// the file name and keeping the extension as a fallback for headerless formats.
fn format_for(path: &Path, bytes: &[u8]) -> Option<ImageFormat> {
    detect_format(bytes).or_else(|| format_from_extension(path))
}

/// Identify an image from its leading bytes, including SVG text.
fn detect_format(bytes: &[u8]) -> Option<ImageFormat> {
    if looks_like_svg(bytes) {
        return Some(ImageFormat::Svg);
    }
    match image::guess_format(bytes).ok()? {
        image::ImageFormat::Png => Some(ImageFormat::Png),
        image::ImageFormat::Jpeg => Some(ImageFormat::Jpeg),
        image::ImageFormat::Gif => Some(ImageFormat::Gif),
        image::ImageFormat::WebP => Some(ImageFormat::Webp),
        image::ImageFormat::Pnm => Some(ImageFormat::Pnm),
        image::ImageFormat::Tiff => Some(ImageFormat::Tiff),
        image::ImageFormat::Bmp => Some(ImageFormat::Bmp),
        image::ImageFormat::Ico => Some(ImageFormat::Ico),
        _ => None,
    }
}

fn format_from_extension(path: &Path) -> Option<ImageFormat> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match extension.as_str() {
        "png" => ImageFormat::Png,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "webp" => ImageFormat::Webp,
        "gif" => ImageFormat::Gif,
        "bmp" => ImageFormat::Bmp,
        "tif" | "tiff" => ImageFormat::Tiff,
        "ico" => ImageFormat::Ico,
        "pbm" | "pgm" | "ppm" | "pnm" => ImageFormat::Pnm,
        "svg" => ImageFormat::Svg,
        _ => return None,
    })
}

/// SVG has no magic number; only its text prologue can identify it, and a
/// document may put a declaration, doctype or comment before the root element.
fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(SVG_SNIFF_BYTES)];
    let text = String::from_utf8_lossy(head);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    if text.starts_with("<svg") {
        return true;
    }
    let prologue =
        text.starts_with("<?xml") || text.starts_with("<!DOCTYPE") || text.starts_with("<!--");
    prologue && text.contains("<svg")
}

fn meta_row(label: SharedString, value: SharedString, cx: &App) -> AnyElement {
    let value_id: ElementId = format!("detail-value:{label}").into();
    let hint = if value.chars().count() > LONG_VALUE_CHARS {
        Some(text_hint(value.clone()))
    } else {
        None
    };
    el! {
        .div { flex; items_center; w_full; min_w_0; gap: MEDIA_RADIUS.px } [
            .div { w: META_LABEL_WIDTH.px; flex_shrink_0; text_xs; text_color: $cx.theme().muted_foreground } [label],
            .div {
                # $(value_id); flex_1; min_w_0; truncate; text_xs; text_ellipsis;
                @if (let Some(hint) = hint) tooltip_show_delay: TOOLTIP_DELAY; tooltip: hint; @end;
            } [value],
        ]
    }
    .into_any_element()
}

fn meta_timestamp(label: SharedString, value: impl IntoElement, cx: &App) -> AnyElement {
    el! {
        .div { flex; items_center; w_full; min_w_0; gap: MEDIA_RADIUS.px } [
            .div { w: META_LABEL_WIDTH.px; flex_shrink_0; text_xs; text_color: $cx.theme().muted_foreground } [label],
            .div { flex_1; min_w_0; text_xs; overflow_hidden } [value],
        ]
    }
    .into_any_element()
}

fn text_hint(text: SharedString) -> impl Fn(&mut Window, &mut App) -> AnyView {
    move |window, cx| Tooltip::new(text.clone()).build(window, cx)
}

impl Browser {
    pub(super) fn preview_target(&self) -> Target {
        let Some(slot) = self.active() else {
            return Target::Empty;
        };
        let Some(tab) = &slot.tab else {
            return Target::Empty;
        };
        let selected = &tab.navigation.view().selected;
        match selected.as_slice() {
            [] => Target::Empty,
            [path] => tab
                .navigation
                .directory()
                .entries
                .iter()
                .find(|entry| entry.path == *path)
                .map_or(Target::Empty, |entry| Target::Item(Item::from_entry(entry))),
            paths => Target::Multiple(paths.len()),
        }
    }

    /// Recompute the preview from current selection state during render.
    pub(super) fn sync_preview(&mut self, cx: &mut Context<Self>) {
        if !self.preview_visible || self.settings_open || self.collection.is_some() {
            return;
        }
        let target = self.preview_target();
        self.preview
            .update(cx, |preview, cx| preview.sync(target, cx));
    }
}

#[cfg(test)]
mod tests;
