use gpui_kit::{AnyElement, Entity, FocusHandle, IntoElement, Subscription};

pub(super) enum Content {
    Settings(Entity<crate::settings::Settings>),
    Catalog(Entity<crate::settings::agents::window::Content>),
    History(Entity<crate::history::Browser>),
    About(Entity<crate::application::About>),
}

pub(super) struct Page {
    pub content: Content,
    pub return_focus: Option<FocusHandle>,
    pub _events: Option<Subscription>,
}

impl Content {
    pub fn sidebar_width(&self) -> Option<f32> {
        match self {
            Self::History(_) => Some(crate::history::SIDEBAR_WIDTH),
            Self::Settings(_) => Some(gds::ui::SIDEBAR_WIDTH),
            Self::Catalog(_) | Self::About(_) => None,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Settings(_) => crate::i18n::tr!("settings-title"),
            Self::Catalog(_) => crate::i18n::tr!("agent-install-action"),
            Self::History(_) => crate::i18n::tr!("agent-history-title"),
            Self::About(_) => crate::i18n::tr!("app-about"),
        }
    }

    pub fn id(&self) -> &'static str {
        match self {
            Self::Settings(_) => "settings",
            Self::Catalog(_) => "agent-installation-page",
            Self::History(_) => "agent-history-window",
            Self::About(_) => "about-window",
        }
    }

    pub fn element(&self) -> AnyElement {
        match self {
            Self::Settings(view) => view.clone().into_any_element(),
            Self::Catalog(view) => view.clone().into_any_element(),
            Self::History(view) => view.clone().into_any_element(),
            Self::About(view) => view.clone().into_any_element(),
        }
    }
}
