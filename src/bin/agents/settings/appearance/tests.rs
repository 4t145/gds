use super::*;
use gpui_kit::{TestAppContext, component::slider::SliderValue, px, size, test::TestWindowExt};

#[gpui_kit::test]
fn background_opacities_apply_save_and_reset_independently(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().expect("isolated appearance");
    let path = directory.path().join("settings.json");
    std::fs::write(
        &path,
        r#"{"appearance":{"theme_source":"builtin","content_opacity":0.74,"sidebar_opacity":0.39}}"#,
    )
    .expect("initial appearance");
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::configuration::init(Some(path.clone()), cx);
        theme::init(cx);
    });
    let mut page = None;
    let window = cx.open_window(size(px(1000.), px(850.)), |window, cx| {
        let editor = theme::Editor::current(cx);
        let view = cx.new(|cx| AppearancePage::new(editor, window, cx));
        page = Some(view.clone());
        Root::new(view, window, cx)
    });
    let page = page.expect("appearance page");
    cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("appearance-opacity-row").visible());
        assert!(window.find("appearance-sidebar-opacity-row").visible());
        page.update(cx, |page, cx| {
            page.apply_number(Field::SidebarOpacity, SliderValue::Single(26.), window, cx);
        });
        assert_eq!(gds::theme::content_background(cx).a, 0.74);
        assert_eq!(gds::theme::sidebar_background(cx).a, 0.26);
        page.update(cx, |page, cx| {
            page.apply_number(Field::Opacity, SliderValue::Single(62.), window, cx);
        });
    })
    .expect("apply both controls");
    let store = crate::configuration::Store::at(path);
    let saved = store.load().expect("saved controls").settings.appearance;
    assert_eq!(saved.content_opacity, Some(0.62));
    assert_eq!(saved.sidebar_opacity, Some(0.26));
    cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("appearance-sidebar-opacity-default", cx);
        assert_eq!(gds::theme::content_background(cx).a, 0.62);
        assert_eq!(
            gds::theme::sidebar_background(cx).a,
            gds::design::SIDEBAR_OPACITY
        );
    })
    .expect("reset sidebar only");
    let saved = store.load().expect("sidebar reset").settings.appearance;
    assert_eq!(saved.content_opacity, Some(0.62));
    assert_eq!(saved.sidebar_opacity, None);
    cx.update_window(window.into(), |_, window, cx| {
        window.click("appearance-opacity-default", cx);
        assert_eq!(
            gds::theme::content_background(cx).a,
            gds::design::CONTENT_OPACITY
        );
    })
    .expect("reset main background");
    let saved = store.load().expect("main reset").settings.appearance;
    assert_eq!(saved.content_opacity, None);
    assert_eq!(saved.sidebar_opacity, None);
}
