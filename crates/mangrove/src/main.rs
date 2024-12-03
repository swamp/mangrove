use mangrove_script::ScriptPlugin;
use std::fmt::Debug;
use swamp::prelude::SwampDefaultPlugins;
use swamp::prelude::*;

#[derive(Debug)]
pub struct ScriptAssets {
    pub x: i32,
}

fn main() {
    App::new()
        .insert_resource(Window {
            title: "mangrove".to_string(),
            requested_surface_size: UVec2::new(640, 480),
            minimal_surface_size: UVec2::new(640, 480),
            fullscreen: false,
        })
        .add_plugins(DefaultPlugins)
        .add_plugins(SwampDefaultPlugins) // these should be in SwampDefaultPlugins
        .add_plugins(ScriptPlugin)
        .run();
}
