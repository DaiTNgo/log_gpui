//! Settings dialog (gpui-kit `Dialog`): auto-tail and shortcuts. Display
//! switches such as density live in Quick Settings on the toolbar.

use crate::app::AppView;
use gpui::{App, Entity, Keystroke, ParentElement, Styled, Window};
use gpui_component::kbd::Kbd;
use gpui_component::switch::Switch;
use gpui_component::{h_flex, v_flex, WindowExt as _};

const SHORTCUTS: [(&str, &str); 11] = [
    ("cmd-o", "New Log Space"),
    ("cmd-s", "Save scratch files"),
    ("cmd-b", "Projects sidebar"),
    ("cmd-alt-b", "Context sidebar"),
    ("cmd-shift-b", "Context: Files"),
    ("cmd-shift-i", "Context: Investigations"),
    ("cmd-shift-s", "Context: Saved Views"),
    ("cmd-p", "Project switcher"),
    ("cmd-k", "Command palette"),
    ("cmd-f", "Search logs"),
    ("cmd-1", "Switch tab"),
];

pub fn open_settings_dialog(view: Entity<AppView>, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _window, cx| {
        let tail = view.read(cx).tail_enabled;

        let tail_view = view.clone();

        let mut shortcuts = v_flex().gap_1();
        for (keys, label) in SHORTCUTS {
            if let Ok(stroke) = Keystroke::parse(keys) {
                shortcuts = shortcuts.child(
                    h_flex().justify_between().child(label).child(Kbd::new(stroke)),
                );
            }
        }

        dialog.title("Settings").w(gpui::px(420.0)).child(
            v_flex()
                .gap_4()
                .child(
                    Switch::new("auto-tail")
                        .label("Auto-tail follow")
                        .checked(tail)
                        .on_click(move |_, _, app| {
                            tail_view.update(app, |this, cx| this.toggle_tail(cx));
                        }),
                )
                .child(v_flex().gap_2().child("Shortcuts").child(shortcuts)),
        )
    });
}
