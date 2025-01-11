/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */

use crate::err::show_mangrove_error;
use crate::script::{compile, MangroveError};
use crate::util::{get_impl_func, get_impl_func_optional};
use crate::{ErrorResource, ScriptMessage, SourceMapResource};
use limnus_clock::Clock;
use limnus_gamepad::{Axis, AxisValueType, Button, ButtonValueType, GamePadId, GamepadMessage};
use monotonic_time_rs::{Millis, MillisDuration, MonotonicClock};
use std::cell::RefCell;
use std::rc::Rc;
use swamp::prelude::{App, Fp, LoRe, LoReM, LocalResource, Msg, Plugin, Re, ReM, UpdatePhase};
use swamp_script::prelude::*;

pub fn logic_tick(
    mut script: LoReM<ScriptLogic>,
    source_map: Re<SourceMapResource>,
    clock: LoRe<Clock>,
    error: Re<ErrorResource>,
) {
    if clock.clock.now() < script.next_time {
        return;
    }

    script.next_time += MillisDuration::from_millis(16);

    let _lookup: &dyn SourceMapLookup = &source_map.wrapper;
    if error.has_errors {
        return;
    }
    script.tick(None).expect("script.tick() crashed");
}

pub fn input_tick(mut script: LoReM<ScriptLogic>, gamepad_messages: Msg<GamepadMessage>) {
    for gamepad_message in gamepad_messages.iter_current() {
        script.gamepad(gamepad_message);
    }
}

#[derive(Debug)]
pub struct ScriptLogicContext {}

#[derive(LocalResource, Debug)]
pub struct ScriptLogic {
    logic_value_ref: ValueRef,
    logic_fn: ResolvedInternalFunctionDefinitionRef,
    gamepad_axis_changed_fn: Option<ResolvedInternalFunctionDefinitionRef>,
    gamepad_button_changed_fn: Option<ResolvedInternalFunctionDefinitionRef>,
    external_functions: ExternalFunctions<ScriptLogicContext>,
    constants: Constants,
    script_context: ScriptLogicContext,
    resolved_program: ResolvedProgram,
    input_module: ResolvedModuleRef,
    next_time: Millis,
}

impl ScriptLogic {
    pub fn new(
        logic_value_ref: ValueRef,
        logic_fn: ResolvedInternalFunctionDefinitionRef,
        gamepad_axis_changed_fn: Option<ResolvedInternalFunctionDefinitionRef>,
        gamepad_button_changed_fn: Option<ResolvedInternalFunctionDefinitionRef>,
        external_functions: ExternalFunctions<ScriptLogicContext>,
        constants: Constants,
        resolved_program: ResolvedProgram,
        //axis_enum_type: ResolvedEnumTypeRef,
        input_module: ResolvedModuleRef,
        now: Millis,
    ) -> Self {
        Self {
            logic_value_ref,
            logic_fn,
            gamepad_axis_changed_fn,
            gamepad_button_changed_fn,
            external_functions,
            script_context: ScriptLogicContext {},
            constants,
            resolved_program,
            input_module,
            next_time: now,
        }
    }

    #[must_use]
    pub fn immutable_logic_value(&self) -> Value {
        self.logic_value_ref.borrow().clone()
    }

    #[must_use]
    pub fn main_module(&self) -> ResolvedModuleRef {
        let root_module_path = &["logic".to_string()].to_vec();

        self.resolved_program
            .modules
            .get(root_module_path)
            .expect("logic module should exist in logic")
    }

    pub fn tick(
        &mut self,
        debug_source_map: Option<&dyn SourceMapLookup>,
    ) -> Result<(), ExecuteError> {
        let variable_value_ref =
            VariableValue::Reference(ValueReference(self.logic_value_ref.clone()));
        let _ = util_execute_function(
            &self.external_functions,
            &self.constants,
            &self.logic_fn,
            &[variable_value_ref],
            &mut self.script_context,
            debug_source_map,
        )?;

        Ok(())
    }

    fn execute(
        &mut self,
        fn_def: &ResolvedInternalFunctionDefinitionRef,
        arguments: &[Value],
    ) -> Result<(), ExecuteError> {
        let mut complete_arguments = Vec::new();
        complete_arguments.push(VariableValue::Reference(ValueReference(
            self.logic_value_ref.clone(),
        ))); // push logic self first
        for arg in arguments {
            complete_arguments.push(VariableValue::Value(arg.clone()));
        }

        let _ = util_execute_function(
            &self.external_functions,
            &self.constants,
            fn_def,
            &complete_arguments,
            &mut self.script_context,
            None,
        )?;

        Ok(())
    }

    pub fn gamepad(&mut self, msg: &GamepadMessage) {
        match msg {
            GamepadMessage::Connected(_, _) => {}
            GamepadMessage::Disconnected(_) => {}
            GamepadMessage::Activated(_) => {}
            GamepadMessage::ButtonChanged(gamepad_id, button, value) => {
                self.button_changed(*gamepad_id, button, value);
            }
            GamepadMessage::AxisChanged(gamepad_id, axis, value) => {
                self.axis_changed(gamepad_id, axis, value);
            }
        }
    }

    fn axis_changed(&mut self, gamepad_id: &GamePadId, axis: &Axis, value: &AxisValueType) {
        let script_axis_value = {
            let input_module_ref = self.input_module.borrow();
            let axis_str = match axis {
                Axis::LeftStickX => "LeftStickX",
                Axis::LeftStickY => "LeftStickY",
                Axis::RightStickX => "RightStickX",
                Axis::RightStickY => "RightStickY",
            };

            let variant = input_module_ref
                .namespace
                .borrow()
                .get_enum_variant_type_str("Axis", axis_str)
                .expect("axis")
                .clone();

            Value::EnumVariantSimple(variant.clone())
        };

        if let Some(found_fn) = &self.gamepad_axis_changed_fn {
            let gamepad_id_value = Value::Int(*gamepad_id as i32);
            let axis_value = Value::Float(Fp::from(*value));

            let fn_ref = found_fn.clone();

            self.execute(&fn_ref, &[gamepad_id_value, script_axis_value, axis_value])
                .expect("gamepad_axis_changed");
        }
    }

    fn button_changed(&mut self, gamepad_id: GamePadId, button: &Button, value: &ButtonValueType) {
        let script_button_value = {
            let input_module_ref = self.input_module.borrow();
            let button_str = match button {
                Button::South => "South",
                Button::East => "East",
                Button::North => "North",
                Button::West => "West",
                Button::LeftTrigger => "LeftTrigger",
                Button::LeftTrigger2 => "LeftTrigger2",
                Button::RightTrigger => "RightTrigger",
                Button::RightTrigger2 => "RightTrigger2",
                Button::Select => "Select",
                Button::Start => "Start",
                Button::Mode => "Mode",
                Button::LeftThumb => "LeftThumb",
                Button::RightThumb => "RightThumb",
                Button::DPadUp => "DPadUp",
                Button::DPadDown => "DPadDown",
                Button::DPadLeft => "DPadLeft",
                Button::DPadRight => "DPadRight",
            };

            let variant = input_module_ref
                .namespace
                .borrow()
                .get_enum_variant_type_str("Button", button_str)
                .expect("button name failed")
                .clone();

            Value::EnumVariantSimple(variant.clone())
        };

        if let Some(found_fn) = &self.gamepad_button_changed_fn {
            let gamepad_id_value = Value::Int(
                i32::try_from(gamepad_id).expect("could not convert gamepad button to i32"),
            );
            let button_value = Value::Float(Fp::from(*value));

            let fn_ref = found_fn.clone();

            self.execute(
                &fn_ref,
                &[gamepad_id_value, script_button_value, button_value],
            )
            .expect("gamepad_button_changed");
        }
    }
}

pub fn input_module(
    resolve_state: &mut ResolvedProgramState,
) -> Result<(ResolvedModule, ResolvedEnumTypeRef, ResolvedEnumTypeRef), ResolveError> {
    let module_path = ["input".to_string()];
    let module = ResolvedModule::new(&module_path);

    let axis_enum_type_ref = {
        let axis_enum_type_id = resolve_state.allocate_number(); // TODO: HACK

        let parent = ResolvedEnumType {
            name: ResolvedLocalTypeIdentifier(ResolvedNode {
                span: Default::default(),
            }),
            assigned_name: "Axis".to_string(),
            module_path: Vec::from(module_path.clone()),
            number: axis_enum_type_id,
        };
        let parent_ref = Rc::new(parent);

        let axis_enum_type_ref = module.namespace.borrow_mut().add_enum_type(parent_ref)?;

        let variant_names = ["LeftStickX", "LeftStickY", "RightStickX", "RightStickY"];
        for variant_name in variant_names {
            let variant_type_id = resolve_state.allocate_number(); // TODO: HACK
            let variant = ResolvedEnumVariantType::new(
                axis_enum_type_ref.clone(),
                ResolvedLocalTypeIdentifier(ResolvedNode {
                    span: Default::default(),
                }),
                variant_name,
                ResolvedEnumVariantContainerType::Nothing,
                variant_type_id,
            );
            module
                .namespace
                .borrow_mut()
                .add_enum_variant("Axis", variant_name, variant)?;
        }
        axis_enum_type_ref
    };

    let button_enum_type_ref = {
        let button_enum_type_id = resolve_state.allocate_number(); // TODO: HACK
                                                                   // let button_enum_type_id = resolve_state.allocate_number(); // TODO: HACK
        let parent = ResolvedEnumType {
            name: ResolvedLocalTypeIdentifier(ResolvedNode {
                span: Default::default(),
            }),
            assigned_name: "Button".to_string(),
            module_path: Vec::from(module_path.clone()),
            number: button_enum_type_id,
        };
        let parent_ref = Rc::new(parent);
        let button_enum_type_ref = module.namespace.borrow_mut().add_enum_type(parent_ref)?;

        let button_names = [
            "South",
            "East",
            "North",
            "West",
            "LeftTrigger",
            "LeftTrigger2",
            "RightTrigger",
            "RightTrigger2",
            "Select",
            "Start",
            "Mode",
            "LeftThumb",
            "RightThumb",
            "DPadUp",
            "DPadDown",
            "DPadLeft",
            "DPadRight",
        ];

        for button_variant_name in button_names {
            let variant_type_id = resolve_state.allocate_number(); // TODO: HACK
            let variant = ResolvedEnumVariantType {
                owner: button_enum_type_ref.clone(),
                data: ResolvedEnumVariantContainerType::Nothing,
                name: ResolvedLocalTypeIdentifier(ResolvedNode {
                    span: Default::default(),
                }),
                assigned_name: button_variant_name.to_string(),
                number: variant_type_id,
            };

            module.namespace.borrow_mut().add_enum_variant(
                "Button",
                button_variant_name,
                variant,
            )?;
        }
        button_enum_type_ref
    };

    Ok((module, axis_enum_type_ref, button_enum_type_ref))
}

pub fn boot(source_map: &mut SourceMapResource, now: Millis) -> Result<ScriptLogic, MangroveError> {
    let mut resolved_program = ResolvedProgram::new();
    let mut external_functions = ExternalFunctions::<ScriptLogicContext>::new();

    let (input_module, _axis_enum_type, _button_enum_type) =
        input_module(&mut resolved_program.state)?;
    let input_module_ref = Rc::new(RefCell::new(input_module));
    resolved_program.modules.add(input_module_ref.clone());

    let base_path = source_map.base_path().to_path_buf();

    compile(
        base_path.as_path(),
        "logic.swamp".as_ref(),
        &["logic".to_string()],
        &mut resolved_program,
        &mut external_functions,
        &mut source_map.wrapper.source_map,
        "logic",
    )?;

    let root_module_path = &["logic".to_string()];
    let main_fn = {
        let main_module = resolved_program
            .modules
            .get(root_module_path)
            .expect("could not find main module");

        let binding = main_module.borrow();
        let function_ref = binding
            .namespace
            .borrow()
            .get_internal_function("main")
            .expect("No main function")
            .clone();

        Rc::clone(&function_ref) // Clone the Rc, not the inner value
    };

    let mut script_context = ScriptLogicContext {};
    resolved_program.modules.finalize()?;
    let mut constants = Constants::new();
    eval_constants(
        &external_functions,
        &mut constants,
        &resolved_program.modules,
        &mut script_context,
    )?;

    let logic_value = util_execute_function(
        &external_functions,
        &constants,
        &main_fn,
        &[],
        &mut script_context,
        None,
    )?;

    let logic_struct_type_ref = if let Value::Struct(struct_type_ref, _) = &logic_value {
        struct_type_ref
    } else {
        return Err(MangroveError::Other("needs to be logic struct".to_string()));
    };

    let logic_fn = get_impl_func(&logic_struct_type_ref, "tick");
    let gamepad_axis_changed_fn =
        get_impl_func_optional(&logic_struct_type_ref, "gamepad_axis_changed");
    let gamepad_button_changed_fn =
        get_impl_func_optional(&logic_struct_type_ref, "gamepad_button_changed");

    // Convert it to a mutable (reference), so it can be mutated in update ticks
    let logic_value_ref = Rc::new(RefCell::new(logic_value));

    Ok(ScriptLogic::new(
        logic_value_ref,
        logic_fn,
        gamepad_axis_changed_fn,
        gamepad_button_changed_fn,
        external_functions,
        constants,
        resolved_program,
        // axis_enum_type,
        input_module_ref.clone(),
        now,
    ))
}

pub fn detect_reload_tick(
    script_messages: Msg<ScriptMessage>,
    mut script_logic: LoReM<ScriptLogic>,
    mut source_map_resource: ReM<SourceMapResource>,
    clock: LoRe<Clock>,
    mut err: ReM<ErrorResource>,
) {
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => match boot(&mut source_map_resource, clock.clock.now()) {
                Ok(new_logic) => *script_logic = new_logic,
                Err(mangrove_error) => {
                    show_mangrove_error(&mangrove_error, &source_map_resource.wrapper.source_map);
                    err.has_errors = true;

                    //                    eprintln!("script logic failed: {}", mangrove_error);
                    //                    error!(error=?mangrove_error, "script logic compile failed");
                }
            },
        }
    }
}

pub struct ScriptLogicPlugin;

impl Plugin for ScriptLogicPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(UpdatePhase::Update, detect_reload_tick);
        app.add_system(UpdatePhase::Update, logic_tick);
        app.add_system(UpdatePhase::Update, input_tick);

        let now = app.local_resources().fetch::<Clock>().clock.now();

        // HACK: Just add a completely zeroed out ScriptLogic and wait for reload message.
        // TODO: Should not try to call updates with params that are not available yet.
        app.insert_local_resource(ScriptLogic {
            logic_value_ref: Rc::new(RefCell::new(Default::default())),
            logic_fn: Rc::new(ResolvedInternalFunctionDefinition {
                body: ResolvedExpression::Break(Default::default()),
                name: ResolvedLocalIdentifier(Default::default()),
                signature: FunctionTypeSignature {
                    first_parameter_is_self: false,
                    parameters: vec![],
                    return_type: Box::from(ResolvedType::Any),
                },
                constants: vec![],
            }),
            gamepad_axis_changed_fn: None,
            gamepad_button_changed_fn: None,
            external_functions: ExternalFunctions::new(),
            constants: Constants { values: vec![] },
            script_context: ScriptLogicContext {},
            resolved_program: ResolvedProgram {
                state: ResolvedProgramState {
                    array_types: vec![],
                    number: 0,
                    external_function_number: 0,
                },
                modules: Default::default(),
            },
            input_module: Rc::new(RefCell::new(ResolvedModule {
                definitions: vec![],
                expression: None,
                namespace: Rc::new(RefCell::new(ResolvedModuleNamespace::new(&vec![]))),
            })),
            next_time: now,
        });
    }
}
