use crate::err::show_mangrove_error;
use crate::script::MangroveError;
use crate::{ErrorResource, ScriptMessage, SourceMapResource};
use std::rc::Rc;
use swamp::prelude::{App, LoReM, LocalResource, Msg, Plugin, PreUpdate, ReM, Update};
use swamp_script::prelude::{
    Constants, ExternalFunctions, InternalFunctionDefinition, InternalFunctionDefinitionRef,
    Program, eval_constants,
};
use tracing::{debug, error};

#[derive(LocalResource, Debug)]
pub struct ScriptMain {
    pub constants: Constants,
    pub resolved_program: Program,
    pub simulation_new_fn: InternalFunctionDefinitionRef,
    pub render_new_fn: InternalFunctionDefinitionRef,
}

impl Default for ScriptMain {
    fn default() -> Self {
        Self {
            constants: Constants::new(),
            resolved_program: Program::default(),
            simulation_new_fn: InternalFunctionDefinitionRef::from(
                InternalFunctionDefinition::default(),
            ),
            render_new_fn: InternalFunctionDefinitionRef::from(
                InternalFunctionDefinition::default(),
            ),
        }
    }
}

pub struct ScriptMainContext {}

#[derive(LocalResource, Debug)]
pub struct ScriptRender {}

pub fn compile(source_map: &mut SourceMapResource) -> Result<ScriptMain, MangroveError> {
    debug!("start compiling");

    let crate_main_path = &["crate".to_string(), "main".to_string()];

    let resolved_program = crate::script::compile(crate_main_path, &mut source_map.source_map)?;

    let simulation_new_fn = {
        let main_module = resolved_program
            .modules
            .get(crate_main_path)
            .expect("could not find main module");

        if let Some(function_ref) = main_module.symbol_table.get_internal_function("simulation") {
            Rc::clone(function_ref)
        } else {
            error!(?main_module.symbol_table, "empty? main module");
            return Err(MangroveError::Other("no main function".to_string()));
        }
    };

    let render_new_fn = {
        let main_module = resolved_program
            .modules
            .get(crate_main_path)
            .expect("could not find main module");

        if let Some(function_ref) = main_module.symbol_table.get_internal_function("render") {
            Rc::clone(function_ref)
        } else {
            error!(?main_module.symbol_table, "empty? main module");
            return Err(MangroveError::Other("no main function".to_string()));
        }
    };

    let external_functions = ExternalFunctions::<ScriptMainContext>::new();

    let mut script_context = ScriptMainContext {};

    let mut constants = Constants::new();
    eval_constants(
        &external_functions,
        &mut constants,
        &resolved_program.state,
        &mut script_context,
    )?;

    let script_game = ScriptMain {
        constants,
        resolved_program,
        simulation_new_fn,
        render_new_fn,
    };

    Ok(script_game)
}

pub fn detect_reload_tick(
    script_messages: Msg<ScriptMessage>,
    mut script_game: LoReM<ScriptMain>,
    mut source_map_resource: ReM<SourceMapResource>,
    mut err: ReM<ErrorResource>,
) {
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => match compile(&mut source_map_resource) {
                Ok(new_simulation) => *script_game = new_simulation,
                Err(mangrove_error) => {
                    err.has_errors = true;
                    show_mangrove_error(&mangrove_error, &source_map_resource.source_map);
                }
            },
        }
    }
}

pub struct ScriptMainPlugin;

impl Plugin for ScriptMainPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(PreUpdate, detect_reload_tick);

        // HACK: Just add a completely zeroed out ScriptGame and wait for reload message.
        // TODO: Should not try to call updates with params that are not available yet.
        app.insert_local_resource(ScriptMain::default());
    }
}
