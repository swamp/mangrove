/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
/*
use crate::err::show_mangrove_error;
use crate::input::ScriptInput;
use crate::script::{compile, MangroveError};
use crate::util::get_impl_func;
use crate::{ErrorResource, ScriptMessage, SourceMapResource};
use std::cell::RefCell;
use std::rc::Rc;
use swamp::prelude::{App, LoRe, LoReM, LocalResource, Msg, Plugin, PreUpdate, Re, ReM, Update};
use swamp_script::prelude::*;

#[derive(Debug, LocalResource)]
struct ScriptFlowInput {
    value: Value,
}

#[derive(Debug)]
pub struct ScriptFlowContext {}

#[derive(LocalResource, Debug)]
pub struct ScriptFlow {
    flow_self_value_ref: ValueRef,
    flow_update_fn: InternalFunctionDefinitionRef,

    //---
    external_functions: ExternalFunctions<ScriptFlowContext>,
    constants: Constants,
    script_context: ScriptFlowContext,
    resolved_program: Program,
    flow_engine_module: ModuleRef,
}

impl Default for ScriptFlow {
    fn default() -> Self {
        Self {
            flow_self_value_ref: Rc::new(RefCell::new(Default::default())),
            flow_update_fn: Rc::new(InternalFunctionDefinition {
                body: Expression {
                    ty: Type::Int,
                    node: Default::default(),
                    kind: ExpressionKind::Break,
                },
                name: LocalIdentifier(Default::default()),
                signature: Signature {
                    parameters: vec![],
                    return_type: Box::new(Type::Int),
                },
            }),
            external_functions: ExternalFunctions::<ScriptFlowContext>::new(),
            constants: Default::default(),
            script_context: ScriptFlowContext {},
            resolved_program: Default::default(),
            flow_engine_module: Rc::new(RefCell::new(Module {
                expression: None,
                namespace: Namespace::default(),
            })),
        }
    }
}

impl ScriptFlow {
    pub fn new(
        flow_self_value_ref: ValueRef,
        flow_update_fn: InternalFunctionDefinitionRef,
        // -----
        external_functions: ExternalFunctions<ScriptFlowContext>,
        constants: Constants,
        resolved_program: Program,
        flow_engine_module: ModuleRef,
    ) -> Self {
        Self {
            flow_self_value_ref,
            flow_update_fn,
            external_functions,
            script_context: ScriptFlowContext {},
            constants,
            resolved_program,
            flow_engine_module,
        }
    }

    /// # Errors
    ///
    pub fn update(
        &mut self,
        flow_input_value: &Value,
        debug_source_map: Option<&dyn SourceMapLookup>,
    ) -> Result<(), ExecuteError> {
        let _ = util_execute_member_function_mut(
            &self.external_functions,
            &self.constants,
            &self.flow_update_fn,
            self.flow_self_value_ref.clone(),
            &[flow_input_value.clone()],
            &mut self.script_context,
            debug_source_map,
        )?;

        Ok(())
    }
}

/// # Errors
///
pub fn create_flow_engine_module(
    _resolve_state: &mut ProgramState,
) -> Result<Module, Error> {
    let module_path = ["mangrove".to_string(), "flow".to_string()];
    let module = Module::new(&module_path);
    Ok(module)
}

pub fn detect_reload_tick(
    script_messages: Msg<ScriptMessage>,
    mut script_flow: LoReM<ScriptFlow>,
    script_input: LoRe<ScriptInput>,
    mut source_map_resource: ReM<SourceMapResource>,
    mut err: ReM<ErrorResource>,
) {
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => {
                match boot(&script_input.main_module(), &mut source_map_resource) {
                    Ok(new_flow) => *script_flow = new_flow,
                    Err(mangrove_error) => {
                        show_mangrove_error(
                            &mangrove_error,
                            &source_map_resource.wrapper.source_map,
                        );
                        err.has_errors = true;

                        //                    eprintln!("script logic failed: {}", mangrove_error);
                        //                    error!(error=?mangrove_error, "script logic compile failed");
                    }
                }
            }
        }
    }
}

/// # Errors
///
/// # Panics
///
pub fn boot(
    input_main_module: &ModuleRef,
    source_map: &mut SourceMapResource,
) -> Result<ScriptFlow, MangroveError> {
    let mut resolved_program = Program::new();
    let mut external_functions = ExternalFunctions::<ScriptFlowContext>::new();

    let flow_engine_module = create_flow_engine_module(&mut resolved_program.state)?;
    let flow_engine_module_ref = Rc::new(RefCell::new(flow_engine_module));
    resolved_program.modules.add(flow_engine_module_ref.clone());
    resolved_program.modules.add(input_main_module.clone());

    let base_path = source_map.base_path().to_path_buf();

    let root_module_path = &["flow".to_string()];

    compile(
        base_path.as_path(),
        root_module_path,
        &mut resolved_program,
        &mut external_functions,
        &mut source_map.wrapper.source_map,
    )?;

    let flow_module = resolved_program
        .modules
        .get(root_module_path)
        .expect("could not find flow module");

    let mut script_context = ScriptFlowContext {};
    let mut constants = Constants::new();
    eval_constants(
        &external_functions,
        &mut constants,
        &resolved_program.modules,
        &mut script_context,
    )?;

    let flow_state_value = {
        let flow_borrow = flow_module.borrow();
        let flow_module_expression = flow_borrow
            .expression
            .as_ref()
            .expect("must have code within the flow module");

        util_execute_expression(
            &external_functions,
            &constants,
            flow_module_expression,
            &mut script_context,
            None,
        )?
    };

    let Value::Struct(logic_struct_type_ref, _) = &flow_state_value else {
        return Err(MangroveError::Other(
            "flow needs to return a struct".to_string(),
        ));
    };

    let flow_update_fn = get_impl_func(logic_struct_type_ref, "update");

    // Convert it to a mutable (reference), so it can be mutated in update ticks
    let flow_state_value_ref = Rc::new(RefCell::new(flow_state_value));

    Ok(ScriptFlow::new(
        flow_state_value_ref,
        flow_update_fn,
        external_functions,
        constants,
        resolved_program,
        flow_engine_module_ref,
    ))
}

/// # Panics
///
pub fn flow_update(
    mut script: LoReM<ScriptFlow>,
    flow_input: LoRe<ScriptFlowInput>,
    _source_map: Re<SourceMapResource>,
    error: Re<ErrorResource>,
) {
    //let lookup: &dyn SourceMapLookup = &source_map.wrapper;
    if error.has_errors {
        return;
    }
    script
        .update(&flow_input.value, None)
        .expect("flow.update() crashed");
}

pub struct ScriptFlowPlugin;

impl Plugin for ScriptFlowPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(PreUpdate, detect_reload_tick);
        app.add_system(Update, flow_update);

        // HACK: Just add a completely zeroed out ScriptFlow and wait for reload message.
        // TODO: Should not try to call updates with params that are not available yet.
        app.insert_local_resource(ScriptFlow::default());
    }
}


 */
