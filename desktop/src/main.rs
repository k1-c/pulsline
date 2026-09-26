//! Pulsline's desktop client. For now it only proves that GPUI opens a
//! window here; the timeline comes in M5 (docs/implementation-plan.md).

use gpui::{
    App, AppContext, Application, Bounds, Context, IntoElement, ParentElement, Render, Styled,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, px, rgb, size,
};

struct Hello;

impl Render for Hello {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .justify_center()
            .items_center()
            .gap_2()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .child(div().text_xl().child("Pulsline"))
            .child(format!("core v{}", pulsline::VERSION))
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(640.), px(400.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Pulsline".into()),
                    ..Default::default()
                }),
                app_id: Some("pulsline".into()),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Hello),
        )
        .expect("failed to open the window");
        cx.activate(true);
    });
}
