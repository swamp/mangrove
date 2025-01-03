/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::source_map::SourceMapWrapper;
use limnus_message::prelude::Message;
use std::path::Path;
use swamp::prelude::{App, Plugin, Resource};

mod audio;
mod err;
pub mod logic;
mod modules;
pub mod render;
mod script;
pub mod source_map;
pub mod temp;
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
    pub fn base_path(&self) -> &Path {
        self.wrapper.source_map.base_path()
    }
}

/*
use crate::script::Script;
use limnus_message::prelude::Message;

use crate::logic::ScriptLogicPlugin;
use crate::modules::ScriptModulesPlugin;
use swamp::prelude::*;





pub fn detect_reload_tick(
    script_messages: Msg<ScriptMessage>,
    mut script_runtime: LoReM<Script>,
    mut all_resources: ReAll,
) {
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => match Script::new(&mut all_resources) {
                Ok(new_compiled_script) => {
                    *script_runtime = new_compiled_script;
                }
                Err(err) => {
                    eprintln!("could not compile: {}", err);
                    error!(err = ?err, "could not compile");
                }
            },
        }
    }
}


pub struct ScriptPlugin;

impl Plugin for ScriptPlugin {
    fn build(&self, app: &mut App) {
        let all_resources = app.resources_mut();
        //let script = Script::new(all_resources).expect("script.boot() crashed");
        app.create_message_type::<ScriptMessage>();
        app.add_plugins(ScriptModulesPlugin);
        app.add_plugins(ScriptLogicPlugin);

        // app.add_system(UpdatePhase::Update, detect_reload_tick);

        app.add_system(UpdatePhase::Update, render_tick);

        app.insert_local_resource(script);
    }
}


 */

pub struct ScriptPlugin;

impl Plugin for ScriptPlugin {
    fn build(&self, app: &mut App) {
        app.create_message_type::<ScriptMessage>();
    }
}
