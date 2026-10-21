use log_gpui::app::AppView;
use gpui::{px, size, AppContext, Bounds, WindowBounds, WindowOptions};
use gpui_component::{theme::ThemeMode, Root, Theme, TitleBar};

fn main() {
    let app = gpui_kit::application().with_assets(gpui_kit::assets::AllAssets);

    app.run(|cx| {
        gpui_kit::init(cx);
        // Default to the dark theme regardless of the system appearance.
        Theme::change(ThemeMode::Dark, None, cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1440.0), px(900.0)),
                cx,
            ))),
            // On Linux the app draws its own title bar and window border
            // (gpui-component's TitleBar and Root); without this the window
            // manager adds its own bar above ours.
            window_decorations: Some(gpui::WindowDecorations::Client),
            // gpui-kit TitleBar owns dragging, double-click zoom and traffic lights.
            ..TitleBar::window_options()
        };

        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                Theme::change(ThemeMode::Dark, Some(window), cx);
                let view = cx.new(|cx| AppView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
