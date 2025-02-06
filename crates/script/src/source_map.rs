/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::SourceMapResource;
use swamp::prelude::{App, Plugin};
use swamp_script::prelude::{SourceMap, SourceMapWrapper};

pub struct SourceMapPlugin;

impl Plugin for SourceMapPlugin {
    fn build(&self, app: &mut App) {
        //app.add_system(UpdatePhase::Update, detect_reload_tick);
        app.insert_resource(SourceMapResource {
            wrapper: SourceMapWrapper {
                source_map: SourceMap::new("scripts/".as_ref()),
            },
        });
    }
}
