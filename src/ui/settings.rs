//! Settings dialog (gpui-kit `Dialog`): row density, auto-tail and shortcuts.

use crate::app::AppView;
use crate::density::Density;
use gpui::{App, Entity, Keystroke, ParentElement, Styled, Window};
use gpui_component::button::{Button, ButtonGroup};
use gpui_component::kbd::Kbd;
use gpui_component::switch::Switch;
use gpui_component::{h_flex, v_flex, Selectable as _, Sizable as _, WindowExt as _};

const SHORTCUTS: [(&str, &str); 9] = [
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
        let density = view.read(cx).density;
        let tail = view.read(cx).tail_enabled;
        let densities = [
            (Density::Comfortable, "Comfortable"),
            (Density::Compact, "Compact"),
            (Density::UltraCompact, "Ultra"),
        ];

        let density_view = view.clone();
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
                    v_flex().gap_2().child("Row density").child(
                        ButtonGroup::new("density")
                            .small()
                            .children(densities.map(|(d, label)| {
                                Button::new(label).label(label).selected(density == d)
                            }))
                            .on_click(move |ix, _, app| {
                                if let Some(&i) = ix.first() {
                                    let d = densities[i].0;
                                    density_view.update(app, |this, cx| this.set_density(d, cx));
                                }
                            }),
                    ),
                )
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
