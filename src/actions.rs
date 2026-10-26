//! Shared semantic actions and default key bindings.

use gpui_kit::*;

gpui_kit::actions!(
    gds,
    [
        ApplicationMenu,
        OpenSettings,
        NewTab,
        CloseTab,
        NextTab,
        PreviousTab,
        ToggleSidebar,
        ToggleTerminal,
        EditLocation,
        NavigateBack,
        NavigateForward,
        NavigateParent,
        Refresh,
        FilterNames,
        ToggleHidden,
        OpenSelection,
        OpenSelectionInNewTabs,
        CopyFiles,
        CutFiles,
        PasteFiles,
        CopyFilePaths,
        SelectAllFiles,
        DeselectAll,
        SelectionMenu,
        RenameSelection,
        ToggleDetails,
        TrashSelection,
        DeleteSelectionPermanently,
        ConfigureColumns,
        FavoriteCurrentFolder,
        RestoreSelection,
        PurgeSelection,
        EmptyTrash,
        ClearRecent,
        MountVolume,
        UnmountVolume,
        TabPrefixHome,
        TabPrefixRecent,
        TabPrefixFavorites,
        TabPrefixNetwork,
        TabPrefixTrash,
        TabPrefix1,
        TabPrefix2,
        TabPrefix3,
        TabPrefix4,
        TabPrefix5,
        TabPrefix6,
        TabPrefix7,
        TabPrefix8,
        TabPrefix9,
        Cancel,
    ]
);

pub const WORKSPACE_CONTEXT: &str = "Workspace";
/// Key context of an armed `Ctrl+Tab` prefix.
///
/// The prefix target keys are bound only here, so they take precedence over the
/// plain workspace bindings for the same strokes (`Ctrl+H`, `Ctrl+D`, ...) and
/// stop applying as soon as the prefix ends.
pub const TAB_PREFIX_CONTEXT: &str = "TabPrefix";
/// Key context of the settings workspace, active while settings replace the
/// file workspace. Bindings here keep shortcuts from leaking between the two.
pub const SETTINGS_CONTEXT: &str = "Settings";
/// Operations on the selected volume or the visible fixed volume page.
pub const VOLUME_CONTEXT: &str = "Volume";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Menu,
    Settings,
    NewTab,
    CloseTab,
    NextTab,
    PreviousTab,
    Sidebar,
    Terminal,
    EditLocation,
    Back,
    Forward,
    Parent,
    Refresh,
    Filter,
    Hidden,
    Favorite,
    Open,
    OpenInNewTab,
    Copy,
    Cut,
    Paste,
    CopyPaths,
    SelectAll,
    DeselectAll,
    ContextMenu,
    Rename,
    Details,
    Columns,
    Trash,
    DeletePermanent,
    RestoreSelection,
    PurgeSelection,
    EmptyTrash,
    ClearRecent,
    MountVolume,
    UnmountVolume,
    Cancel,
}

impl Command {
    pub const ALL: [Self; 36] = [
        Self::Menu,
        Self::Settings,
        Self::NewTab,
        Self::CloseTab,
        Self::NextTab,
        Self::PreviousTab,
        Self::Sidebar,
        Self::Terminal,
        Self::EditLocation,
        Self::Back,
        Self::Forward,
        Self::Parent,
        Self::Refresh,
        Self::Filter,
        Self::Hidden,
        Self::Favorite,
        Self::Open,
        Self::OpenInNewTab,
        Self::Copy,
        Self::Cut,
        Self::Paste,
        Self::CopyPaths,
        Self::SelectAll,
        Self::DeselectAll,
        Self::ContextMenu,
        Self::Rename,
        Self::Details,
        Self::Columns,
        Self::Trash,
        Self::DeletePermanent,
        Self::RestoreSelection,
        Self::PurgeSelection,
        Self::EmptyTrash,
        Self::ClearRecent,
        Self::MountVolume,
        Self::UnmountVolume,
    ];

    pub fn label_id(self) -> &'static str {
        match self {
            Self::Menu => "app-menu",
            Self::Settings => "settings",
            Self::NewTab => "new-tab",
            Self::CloseTab => "close-tab",
            Self::NextTab => "next-tab",
            Self::PreviousTab => "previous-tab",
            Self::Sidebar => "sidebar-toggle",
            Self::Terminal => "terminal-toggle",
            Self::EditLocation => "path-edit",
            Self::Back => "back",
            Self::Forward => "forward",
            Self::Parent => "parent",
            Self::Refresh => "refresh",
            Self::Filter => "filter",
            Self::Hidden => "show-hidden",
            Self::Favorite => "favorite-toggle",
            Self::Open => "open",
            Self::OpenInNewTab => "open-in-new-tab",
            Self::Copy => "copy-files",
            Self::Cut => "cut-files",
            Self::Paste => "paste-files",
            Self::CopyPaths => "copy-path",
            Self::SelectAll => "select-all",
            Self::DeselectAll => "deselect-all",
            Self::ContextMenu => "selection-menu",
            Self::Rename => "rename",
            Self::Details => "details-toggle",
            Self::Columns => "columns-configure",
            Self::Trash => "move-to-trash",
            Self::DeletePermanent => "delete-permanently",
            Self::RestoreSelection => "recycle-restore",
            Self::PurgeSelection => "delete-permanently",
            Self::EmptyTrash => "recycle-empty-action",
            Self::ClearRecent => "clear-recent",
            Self::MountVolume => "device-mount",
            Self::UnmountVolume => "device-unmount",
            Self::Cancel => "close",
        }
    }

    pub fn action(self) -> Box<dyn Action> {
        match self {
            Self::Menu => Box::new(ApplicationMenu),
            Self::Settings => Box::new(OpenSettings),
            Self::NewTab => Box::new(NewTab),
            Self::CloseTab => Box::new(CloseTab),
            Self::NextTab => Box::new(NextTab),
            Self::PreviousTab => Box::new(PreviousTab),
            Self::Sidebar => Box::new(ToggleSidebar),
            Self::Terminal => Box::new(ToggleTerminal),
            Self::EditLocation => Box::new(EditLocation),
            Self::Back => Box::new(NavigateBack),
            Self::Forward => Box::new(NavigateForward),
            Self::Parent => Box::new(NavigateParent),
            Self::Refresh => Box::new(Refresh),
            Self::Filter => Box::new(FilterNames),
            Self::Hidden => Box::new(ToggleHidden),
            Self::Favorite => Box::new(FavoriteCurrentFolder),
            Self::Open => Box::new(OpenSelection),
            Self::OpenInNewTab => Box::new(OpenSelectionInNewTabs),
            Self::Copy => Box::new(CopyFiles),
            Self::Cut => Box::new(CutFiles),
            Self::Paste => Box::new(PasteFiles),
            Self::CopyPaths => Box::new(CopyFilePaths),
            Self::SelectAll => Box::new(SelectAllFiles),
            Self::DeselectAll => Box::new(DeselectAll),
            Self::ContextMenu => Box::new(SelectionMenu),
            Self::Rename => Box::new(RenameSelection),
            Self::Details => Box::new(ToggleDetails),
            Self::Columns => Box::new(ConfigureColumns),
            Self::Trash => Box::new(TrashSelection),
            Self::DeletePermanent => Box::new(DeleteSelectionPermanently),
            Self::RestoreSelection => Box::new(RestoreSelection),
            Self::PurgeSelection => Box::new(PurgeSelection),
            Self::EmptyTrash => Box::new(EmptyTrash),
            Self::ClearRecent => Box::new(ClearRecent),
            Self::MountVolume => Box::new(MountVolume),
            Self::UnmountVolume => Box::new(UnmountVolume),
            Self::Cancel => Box::new(Cancel),
        }
    }

    pub fn context(self) -> Option<&'static str> {
        match self {
            Self::Settings | Self::Terminal | Self::Sidebar | Self::Cancel => None,
            Self::MountVolume | Self::UnmountVolume => Some(VOLUME_CONTEXT),
            _ => Some(WORKSPACE_CONTEXT),
        }
    }

    pub fn binding(self, window: &Window) -> Option<KeyBinding> {
        let action = self.action();
        let context = self
            .context()
            .map(|context| KeyContext::parse(context).expect("bundled action contexts are valid"))
            .unwrap_or_default();
        window.highest_precedence_binding_for_action_in_context(action.as_ref(), context)
    }

    pub fn matches_single_stroke(self, stroke: &Keystroke, window: &Window) -> bool {
        let Some(binding) = self.binding(window) else {
            return false;
        };
        if binding.keystrokes().len() != 1 {
            return false;
        }
        let expected = binding.keystrokes()[0].as_keystroke();
        expected.key == stroke.key && expected.modifiers == stroke.modifiers
    }

    /// The default keystroke for this command, as registered by [`register`].
    pub fn default_stroke(self) -> &'static str {
        match self {
            Self::Menu => "alt-m",
            Self::Settings => "ctrl-,",
            Self::NewTab => "ctrl-n",
            Self::CloseTab => "ctrl-w",
            Self::NextTab => "ctrl-tab",
            Self::PreviousTab => "ctrl-shift-tab",
            Self::Sidebar => "ctrl-b",
            Self::Terminal => "ctrl-`",
            Self::EditLocation => "ctrl-l",
            Self::Back => "alt-left",
            Self::Forward => "alt-right",
            Self::Parent => "alt-up",
            Self::Refresh => "ctrl-r",
            Self::Filter => "ctrl-f",
            Self::Hidden => "ctrl-h",
            Self::Favorite => "ctrl-d",
            Self::Open => "enter",
            Self::OpenInNewTab => "ctrl-enter",
            Self::Copy => "ctrl-c",
            Self::Cut => "ctrl-x",
            Self::Paste => "ctrl-v",
            Self::CopyPaths => "ctrl-shift-c",
            Self::SelectAll => "ctrl-a",
            Self::DeselectAll => "ctrl-shift-a",
            Self::ContextMenu => "shift-f10",
            Self::Rename => "f2",
            Self::Details => "ctrl-p",
            Self::Columns => "alt-v",
            Self::Trash => "delete",
            Self::DeletePermanent => "shift-delete",
            Self::RestoreSelection => "alt-r",
            Self::PurgeSelection => "alt-d",
            Self::EmptyTrash => "alt-shift-d",
            Self::ClearRecent => "alt-c",
            Self::MountVolume => "ctrl-m",
            Self::UnmountVolume => "ctrl-shift-m",
            Self::Cancel => "escape",
        }
    }

    /// Extra strokes that run the same action in the same key context.
    ///
    /// [`register`] adds them before [`Self::default_stroke`], so the primary
    /// stroke stays the highest-precedence binding that keycaps, tooltips,
    /// Alt hints and the settings page display.
    pub fn compatibility_strokes(self) -> &'static [&'static str] {
        match self {
            // F5 is the historical refresh key; Ctrl+R is the primary one.
            Self::Refresh => &["f5"],
            // F10 is the conventional menu key; Alt+M is the primary one.
            Self::Menu => &["f10"],
            _ => &[],
        }
    }
}

pub fn register(cx: &mut App) {
    cx.bind_keys(bindings());
    crate::ui::dialog::register(cx);
}

/// Every bundled binding, in precedence order: compatibility aliases first,
/// then primary strokes. The last binding for an action is the one displayed.
fn bindings() -> Vec<KeyBinding> {
    let compatibility = Command::ALL.into_iter().flat_map(|command| {
        command
            .compatibility_strokes()
            .iter()
            .copied()
            .map(move |stroke| binding(stroke, command, command.context()))
    });
    let primary = Command::ALL
        .into_iter()
        .chain([Command::Cancel])
        .map(|command| binding(command.default_stroke(), command, command.context()));
    // Ctrl+Tab cycles settings pages while the settings workspace has
    // the key context; the file workspace keeps its own tab bindings.
    let settings = [
        (Command::NextTab, "ctrl-tab"),
        (Command::PreviousTab, "ctrl-shift-tab"),
    ]
    .into_iter()
    .map(|(command, stroke)| binding(stroke, command, Some(SETTINGS_CONTEXT)));
    // Registered last: at equal context depth the later binding wins, and the
    // prefix targets have to beat the plain `Ctrl+H`/`Ctrl+D`/... workspace
    // bindings while the prefix is armed.
    compatibility
        .chain(primary)
        .chain(settings)
        .chain(tab_prefix_bindings())
        // Popup cancellation must win over the workspace's global Escape.
        // Otherwise it moves focus behind a menu while leaving that menu open.
        .chain([
            action_binding(
                "escape",
                Box::new(gpui_kit::base::actions::Cancel),
                Some("PopupMenu"),
            ),
            action_binding(
                "enter",
                Box::new(gpui_kit::base::actions::Confirm { secondary: false }),
                Some("PopupMenu"),
            ),
        ])
        .collect()
}

/// The page keys staged behind an armed `Ctrl+Tab` prefix.
///
/// `Ctrl+Tab` itself is still [`Command::NextTab`]: it arms the prefix instead
/// of cycling, and only a release without a target key cycles. These bindings
/// exist only in [`TAB_PREFIX_CONTEXT`], so the same strokes keep their plain
/// meaning outside the prefix. Shift variants keep the targets reachable while
/// `Ctrl+Shift+Tab` started the prefix.
fn tab_prefix_bindings() -> Vec<KeyBinding> {
    let context = Some(TAB_PREFIX_CONTEXT);
    vec![
        action_binding("ctrl-shift-h", Box::new(TabPrefixHome), context),
        action_binding("ctrl-shift-r", Box::new(TabPrefixRecent), context),
        action_binding("ctrl-shift-d", Box::new(TabPrefixFavorites), context),
        action_binding("ctrl-shift-n", Box::new(TabPrefixNetwork), context),
        action_binding("ctrl-shift-b", Box::new(TabPrefixTrash), context),
        action_binding("ctrl-shift-1", Box::new(TabPrefix1), context),
        action_binding("ctrl-shift-2", Box::new(TabPrefix2), context),
        action_binding("ctrl-shift-3", Box::new(TabPrefix3), context),
        action_binding("ctrl-shift-4", Box::new(TabPrefix4), context),
        action_binding("ctrl-shift-5", Box::new(TabPrefix5), context),
        action_binding("ctrl-shift-6", Box::new(TabPrefix6), context),
        action_binding("ctrl-shift-7", Box::new(TabPrefix7), context),
        action_binding("ctrl-shift-8", Box::new(TabPrefix8), context),
        action_binding("ctrl-shift-9", Box::new(TabPrefix9), context),
        action_binding("ctrl-h", Box::new(TabPrefixHome), context),
        action_binding("ctrl-r", Box::new(TabPrefixRecent), context),
        action_binding("ctrl-d", Box::new(TabPrefixFavorites), context),
        action_binding("ctrl-n", Box::new(TabPrefixNetwork), context),
        action_binding("ctrl-b", Box::new(TabPrefixTrash), context),
        action_binding("ctrl-1", Box::new(TabPrefix1), context),
        action_binding("ctrl-2", Box::new(TabPrefix2), context),
        action_binding("ctrl-3", Box::new(TabPrefix3), context),
        action_binding("ctrl-4", Box::new(TabPrefix4), context),
        action_binding("ctrl-5", Box::new(TabPrefix5), context),
        action_binding("ctrl-6", Box::new(TabPrefix6), context),
        action_binding("ctrl-7", Box::new(TabPrefix7), context),
        action_binding("ctrl-8", Box::new(TabPrefix8), context),
        action_binding("ctrl-9", Box::new(TabPrefix9), context),
    ]
}

fn binding(stroke: &'static str, command: Command, context: Option<&'static str>) -> KeyBinding {
    action_binding(stroke, command.action(), context)
}

fn action_binding(
    stroke: &'static str,
    action: Box<dyn Action>,
    context: Option<&'static str>,
) -> KeyBinding {
    let predicate = context.map(|value| {
        KeyBindingContextPredicate::parse(value)
            .expect("bundled action contexts are valid")
            .into()
    });
    KeyBinding::load(stroke, action, predicate, false, None, &DummyKeyboardMapper)
        .expect("bundled action shortcuts are valid")
}

#[cfg(test)]
mod tests;
