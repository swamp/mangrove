/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */

use mangrove_save_detector::SaveDetectorPlugin;
use mangrove_script::logic::ScriptLogicPlugin;
use mangrove_script::render::ScriptRenderPlugin;
use mangrove_script::source_map::SourceMapPlugin;
use mangrove_script::temp::FlushRenderPlugin;
use mangrove_script::ScriptPlugin;
use swamp::prelude::SwampDefaultPlugins;
use swamp::prelude::*;

//const RESOLUTION: (u16, u16) = (640, 360);
const RESOLUTION: (u16, u16) = (640 / 2, 360 / 2);

fn main() {
    App::new()
        .insert_resource(Window {
            title: "mangrove".to_string(),
            requested_surface_size: UVec2::new(RESOLUTION.0 * 6, RESOLUTION.1 * 6),
            minimal_surface_size: UVec2::new(RESOLUTION.0, RESOLUTION.1),
            fullscreen: false,
        })
        .add_plugins(DefaultPlugins)
        .add_plugins(SwampDefaultPlugins)
        .add_plugins(FlushRenderPlugin)
        .add_plugins(SourceMapPlugin)
        .add_plugins(ScriptPlugin)
        .add_plugins(ScriptLogicPlugin)
        .add_plugins(ScriptRenderPlugin)
        .add_plugins(SaveDetectorPlugin)
        .run();
}
