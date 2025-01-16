/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::source_map::SourceMapWrapper;
use limnus_message::prelude::Message;
use std::path::Path;
use swamp::prelude::{App, Plugin, Resource};

mod audio;
pub mod err;
pub mod logic;
mod modules;
pub mod render;
mod script;
pub mod serialize;
pub mod source_map;
mod util;

#[derive(Message, Debug)]
pub enum ScriptMessage {
    Reload,
}

#[derive(Resource, Debug)]
pub struct SourceMapResource {
    pub wrapper: SourceMapWrapper,
}

impl SourceMapResource {
    #[must_use]
    pub fn base_path(&self) -> &Path {
        self.wrapper.source_map.base_path()
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
