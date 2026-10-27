//! Shared content-page geometry for directories and collection tables.

use gds_uim::el;
use gpui_kit::{prelude::*, *};

use crate::typography;

const HEADER_PADDING_X: f32 = 24.;
const HEADER_PADDING_Y: f32 = 16.;
const TITLE_SIZE: f32 = 22.;
const ACTION_GAP: f32 = 8.;

/// A fixed heading and action row above a flexible content area.
/// Actions keep their own command, tooltip and key-hint behavior.
#[derive(IntoElement)]
pub struct ContentPage {
    id: ElementId,
    title: SharedString,
    actions: Vec<AnyElement>,
    notice: Option<AnyElement>,
    body: AnyElement,
}

impl ContentPage {
    pub const TABLE_INSET: f32 = 18.;

    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        body: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            actions: Vec::new(),
            notice: None,
            body: body.into_any_element(),
        }
    }

    pub fn actions(mut self, actions: Vec<AnyElement>) -> Self {
        self.actions = actions;
        self
    }

    pub fn notice(mut self, notice: Option<AnyElement>) -> Self {
        self.notice = notice;
        self
    }

    /// Keep table insets inside the selection surface so margins clear selection.
    pub fn table_area(id: impl Into<ElementId>, table: impl IntoElement) -> Stateful<Div> {
        let id = id.into();
        el! {
            .div { # id; flex; flex_col; flex_1; min_h_0; min_w_0; overflow_hidden;
                px: Self::TABLE_INSET.px;
            } [table]
        }
    }
}

impl RenderOnce for ContentPage {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            title,
            actions,
            notice,
            body,
        } = self;
        el! {
            .div { # id; flex; flex_col; flex_1; size_full; min_h_0; min_w_0 } [
                .div { flex; flex_shrink_0; items_center; gap: ACTION_GAP.px;
                    px: HEADER_PADDING_X.px; py: HEADER_PADDING_Y.px;
                } [
                    .div { flex_1; min_w_0; truncate; font_weight: FontWeight::SEMIBOLD;
                        text_size: $(typography::scaled(TITLE_SIZE, cx));
                    } [title],
                    ..actions,
                ],
                @if (let Some(notice) = notice)
                    .div { flex_shrink_0; min_w_0; px: HEADER_PADDING_X.px; pb: ACTION_GAP.px } [notice],
                @end,
                body,
            ]
        }
    }
}
