/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::SourceMapResource;
use seq_map::SeqMap;
use std::path::Path;
use swamp::prelude::{App, Plugin};
use swamp_script::prelude::{SourceMap, SourceMapWrapper};

pub struct SourceMapPlugin;

impl Plugin for SourceMapPlugin {
    fn build(&self, app: &mut App) {
        //app.add_system(UpdatePhase::Update, detect_reload_tick);
        let mut mounts = SeqMap::new();
        let path_buf = Path::new("scripts/").to_path_buf();
        mounts.insert("crate".to_string(), path_buf).unwrap();
        app.insert_resource(SourceMapResource {
            wrapper: SourceMapWrapper {
                source_map: SourceMap::new(&mounts).unwrap(),
            },
        });
    }
}
