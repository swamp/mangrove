use monotonic_time_rs::{InstantMonotonicClock, MonotonicClock};
use std::fmt::{Debug, Formatter};
use swamp::prelude::*;

// Should be moved to Swamp Engine
pub fn flush_render_tick(
    script: LoRe<Clock>,
    wgpu_window: Re<WgpuWindow>,
    mut wgpu_render: ReM<Render>,
    materials: Re<LimnusAssets<Material>>,
    fonts: Re<LimnusAssets<Font>>,
) {
    let now = script.clock.now();

    wgpu_window
        .render(wgpu_render.clear_color(), |render_pass| {
            wgpu_render.render(render_pass, &materials, &fonts, now)
        })
        .unwrap();
}

#[derive(LocalResource)]
pub struct Clock {
    pub clock: InstantMonotonicClock,
}

impl Debug for Clock {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "clock")
    }
}

pub struct FlushRenderPlugin;

impl Plugin for FlushRenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_local_resource(Clock {
            clock: InstantMonotonicClock::new(),
        });
        app.add_system(UpdatePhase::Update, flush_render_tick);
    }
}
