/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::err::show_mangrove_error;
use crate::script::MangroveError;
use crate::simulation::ScriptSimulation;
use crate::util::ScriptModule;
use crate::{util, ErrorResource, SourceMapResource};
use swamp::prelude::{App, LoRe, LoReM, LocalResource, Plugin, PreUpdate, Re};
use swamp_script::prelude::*;

/// # Panics
///
pub fn input_tick(
    mut script: LoReM<ScriptInput>,
    simulation: LoRe<ScriptSimulation>,
    _source_map: Re<SourceMapResource>,
    error: Re<ErrorResource>,
) {
    //let lookup: &dyn SourceMapLookup = &source_map.wrapper;
    if error.has_errors {
        return;
    }
    script.tick(simulation.immutable_simulation_value(), None);
}

#[derive(Debug)]
pub struct ScriptInputContext {}

#[derive(LocalResource, Debug)]
pub struct ScriptInput {
    pub script_updater: ScriptModule<ScriptInputContext>,
    pub converted_simulation_value: Value,
}

impl ScriptInput {
    pub fn new(script_module: ScriptModule<ScriptInputContext>) -> Self {
        Self {
            script_updater: script_module,
            converted_simulation_value: Value::Unit,
        }
    }

    /// # Panics
    ///
    #[must_use]
    pub fn main_module(&self) -> ResolvedModuleRef {
        self.script_updater.main_module()
    }

    pub fn tick(
        &mut self,
        simulation_value: Value,
        debug_source_map: Option<&dyn SourceMapLookup>,
    ) {
        self.simulation_input(simulation_value, debug_source_map)
            .expect("simulation input failed");
    }

    /// # Errors
    ///
    pub fn simulation_input(
        &mut self,
        logic: Value,
        debug_source_map: Option<&dyn SourceMapLookup>,
    ) -> Result<(), ExecuteError> {
        let _ = self
            .script_updater
            .update(&[logic.clone()], debug_source_map);

        Ok(())
    }
}

/// # Errors
///
/// # Panics
///
pub fn boot(
    simulation_main_module: &ResolvedModuleRef,
    flow_main_module: &ResolvedModuleRef,
    source_map: &mut SourceMapResource,
) -> Result<ScriptInput, MangroveError> {
    let updater = util::boot(
        vec![simulation_main_module, flow_main_module],
        &["simulation_input".to_string()],
        "update",
        ScriptInputContext {},
        source_map,
    )?;

    Ok(ScriptInput::new(updater))
}

pub struct ScriptInputPlugin;

impl Plugin for ScriptInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(PreUpdate, input_tick);
        let source_map_resource = app
            .get_resource_mut::<SourceMapResource>()
            .expect("must have source map resource");

        /* let script_input = boot(source_map_resource);
        if let Err(mangrove_error) = &script_input {
            show_mangrove_error(mangrove_error, &source_map_resource.wrapper.source_map);
        }
        app.insert_local_resource(script_input.unwrap());

        */
    }
}
