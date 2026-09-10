pub mod app;
pub mod column;
pub mod filter;
pub mod infrastructure;
pub mod marker;
pub mod model;
pub mod parser;
pub mod query;
pub mod sample;
pub mod search;
pub mod theme;
pub mod ui;

use app::AppView;
use gpui::{px, size, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions};
use gpui_component::Root;

fn main() {
    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);

    app.run(|cx| {
        gpui_kit::init(cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1440.0), px(900.0)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("Log Editor (Zed Minimalism)".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|cx| AppView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
