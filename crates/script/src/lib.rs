/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use limnus_message::prelude::Message;
use std::path::Path;
use swamp::prelude::{App, Plugin, Resource};

mod audio;
pub mod convert;
pub mod err;
pub mod flow;
pub mod input;
mod modules;
pub mod render;
mod script;
pub mod script_main;
pub mod serialize;
mod sim_input;
pub mod simulation;
pub mod source_map;
mod util;

use swamp_script::prelude::{SourceMap, SourceMapWrapper};

#[derive(Message, Debug)]
pub enum ScriptMessage {
    Reload,
}

#[derive(Resource, Debug)]
pub struct SourceMapResource {
    pub source_map: SourceMap,
}

impl SourceMapResource {
    pub fn wrapper(&self) -> SourceMapWrapper {
        SourceMapWrapper {
            source_map: &self.source_map,
        }
    }
}

impl SourceMapResource {
    #[must_use]
    pub fn base_path(&self) -> &Path {
        self.wrapper().source_map.base_path("crate")
    }
}

#[derive(Resource, Debug)]
pub struct ErrorResource {
    pub has_errors: bool,
}

pub struct ScriptPlugin;

impl Plugin for ScriptPlugin {
    fn build(&self, app: &mut App) {
        app.create_message_type::<ScriptMessage>();

        let script_messages = app
            .messages_mut()
            .get_mut::<ScriptMessage>()
            .expect("missing ScriptMessage");
        script_messages.send(ScriptMessage::Reload);
    }
}
