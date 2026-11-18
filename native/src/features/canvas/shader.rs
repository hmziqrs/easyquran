//! The `shader` element: an animated WGSL program drawing its box, its
//! tint uniform bound to the theme, with pause and drift controls. Where
//! no GPU renderer runs (or the source does not compile), the view shows
//! its background and children — the graceful fallback the page keeps
//! styled.

use std::sync::OnceLock;

use nsh_base::theme;
use nsh_core::{
    prelude::*,
    shader::{Shader, ShaderError},
};

use super::{caption, section, tool_button};

/// The program: two drifting aurora bands shaded over the box, its clock
/// (`nsh.time`) running unless paused. `drift` redraws it even when
/// paused: uniforms still draw a paused shader.
const AURORA: &str = r#"
struct Uniforms {
    tint: vec3f,
    drift: f32,
}

fn band(uv: vec2f, phase: f32, drift: f32) -> f32 {
    let w = uv.x * 2.4 + nsh.time * drift + phase;
    let crest = sin(w) * 0.5 + 0.5;
    return 1.0 - smoothstep(0.1, 0.85, uv.y - crest * 0.45);
}

fn shade(uv: vec2f) -> vec4f {
    let a = band(uv, 0.0, uniforms.drift) * 0.7;
    let b = band(uv, 1.7, uniforms.drift * 1.4) * 0.5;
    let glow = clamp(a + b, 0.0, 1.0);
    return vec4f(uniforms.tint * (0.55 + 0.45 * glow), glow * 0.9);
}
"#;

/// Compiled once; views sharing the source share the program.
fn program() -> Result<Shader, ShaderError> {
    static PROGRAM: OnceLock<Result<Shader, ShaderError>> = OnceLock::new();
    PROGRAM.get_or_init(|| Shader::compile(AURORA)).clone()
}

/// The shader section: the aurora with its caption over it, its tint
/// following the theme's third chart color across appearances.
#[component]
pub(crate) fn ShaderDemo() -> impl IntoElement {
    let paused = signal(false);
    let drift = signal(0.7_f32);
    let stage = match program() {
        Ok(compiled) => shader()
            .source(compiled)
            .uniform("tint", move || color(theme().chart_3))
            .uniform("drift", move || drift.get())
            .paused(move || paused.get())
            .h(px(170.0))
            .rounded(move || theme().radius)
            .overflow_hidden()
            .bg(move || theme().muted)
            .test_id("shader-view")
            .label("Aurora shader")
            .child(
                text("Aurora — WGSL over this box")
                    .text_xs()
                    .text_color(move || theme().foreground.opacity(0.7)),
            )
            .into_element(),
        // No compiled program: what a platform without the GPU renderer
        // shows, kept in the same shape.
        Err(_) => view()
            .h(px(170.0))
            .rounded(move || theme().radius)
            .bg(move || theme().muted)
            .items_center()
            .justify_center()
            .child(
                text("Shader unavailable — background fallback")
                    .text_xs()
                    .text_color(move || theme().muted_foreground),
            )
            .into_element(),
    };
    view().child(section(
        "Shader",
        "WGSL · wgpu on iOS, Android, macOS, Windows, Linux · WebGL 2 on the web",
        view()
            .gap_3()
            .child(stage)
            .child(
                view()
                    .flex_row()
                    .gap_2()
                    .child(view().test_id("shader-pause").child(tool_button(
                        move || {
                            if paused.get() { "Resume" } else { "Pause" }
                        },
                        false,
                        move || paused.update(|p| *p = !*p),
                    )))
                    .child(tool_button("Calm", false, move || drift.set(0.2)))
                    .child(tool_button("Swift", false, move || drift.set(1.6))),
            )
            .child(caption("shader-note", move || {
                "An animated shader draws every frame; children draw over it.".to_owned()
            })),
    ))
}
