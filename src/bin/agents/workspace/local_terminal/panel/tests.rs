use super::{NewTerminal, Panel, Preferences};
use gpui_kit::{AppContext, Entity, TestAppContext, component::Root, px, size};

#[gpui_kit::test]
async fn agents_selection_and_new_target_do_not_retarget_existing_terminals(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let directory = tempfile::tempdir().expect("terminal test directory");
    let first_cwd = directory.path().join("first");
    let second_cwd = directory.path().join("second");
    std::fs::create_dir(&first_cwd).expect("first directory");
    std::fs::create_dir(&second_cwd).expect("second directory");
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::theme::init_default(cx);
        super::super::bind_keys(cx);
    });
    let mut panel: Option<Entity<Panel>> = None;
    let window = cx.open_window(size(px(900.), px(500.)), |window, cx| {
        let view = cx.new(Panel::new);
        view.update(cx, |panel, cx| {
            // A missing explicit shell exercises managed startup without opening
            // a user's shell or depending on its interactive configuration.
            panel.set_preferences(
                Preferences {
                    shell: Some(directory.path().join("missing-test-shell")),
                    ..Default::default()
                },
                cx,
            );
        });
        panel = Some(view.clone());
        Root::new(view, window, cx)
    });
    let panel = panel.expect("collection");
    let shutdown = window
        .update(cx, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.set_cwd(Some(first_cwd.clone()), cx);
                panel.new_terminal(&NewTerminal, window, cx);
                let first = panel.entries[0].terminal.clone();
                assert_eq!(first.read(cx).cwd(), first_cwd.as_path());
                panel.set_cwd(Some(second_cwd.clone()), cx);
                panel.new_terminal(&NewTerminal, window, cx);
                assert_eq!(panel.entries.len(), 2);
                assert_eq!(first.read(cx).cwd(), first_cwd.as_path());
                assert_eq!(
                    panel.entries[1].terminal.read(cx).cwd(),
                    second_cwd.as_path()
                );
                let second_id = panel.entries[1].id;
                assert_eq!(panel.active, Some(second_id));
                panel.cycle(true, window, cx);
                assert_eq!(
                    panel.active_terminal().expect("first selected").entity_id(),
                    first.entity_id()
                );
                panel.set_cwd(None, cx);
                panel.new_terminal(&NewTerminal, window, cx);
                assert_eq!(
                    panel.entries.len(),
                    2,
                    "unknown cwd disables only future creation"
                );
                let shutdown = panel.shutdown(cx);
                panel.open(first_cwd.clone(), window, cx);
                assert!(!panel.has_sessions(), "shutdown seals the collection");
                shutdown
            })
        })
        .expect("manage local terminal selection");
    shutdown.await;
}
