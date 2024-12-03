use mangrove_script::ScriptPlugin;
use std::fmt::Debug;
use swamp::prelude::SwampDefaultPlugins;
use swamp::prelude::*;

fn main() {
    App::new()
        .insert_resource(Window {
            title: "mangrove".to_string(),
            requested_surface_size: UVec2::new(320 * 4, 240 * 4),
            minimal_surface_size: UVec2::new(320, 240),
            fullscreen: false,
        })
        .add_plugins(DefaultPlugins)
        .add_plugins(SwampDefaultPlugins) // these should be in SwampDefaultPlugins
        .add_plugins(ScriptPlugin)
        .run();
}
