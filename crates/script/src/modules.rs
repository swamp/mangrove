/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use swamp::prelude::{App, LocalResource, Plugin};
use swamp_script::prelude::*;

#[derive(LocalResource, Debug)]
pub struct ScriptModules {
    #[allow(unused)]
    pub modules: ResolvedModules,
}

pub struct ScriptModulesPlugin;

impl Plugin for ScriptModulesPlugin {
    fn build(&self, app: &mut App) {
        let script_modules = ScriptModules {
            modules: ResolvedModules::default(),
        };
        app.insert_local_resource(script_modules);
    }
}
