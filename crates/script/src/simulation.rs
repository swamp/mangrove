/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::err::show_mangrove_error;
use crate::script::{MangroveError, compile};
use crate::util::{get_impl_func, get_impl_func_optional};
use crate::{ErrorResource, ScriptMessage, SourceMapResource};
use limnus_gamepad::{Axis, AxisValueType, Button, ButtonValueType, GamePadId, GamepadMessage};
use std::cell::RefCell;
use std::rc::Rc;
use swamp::prelude::{App, Fp, LoReM, LocalResource, Msg, Plugin, PreUpdate, Re, ReM, Update};
use swamp_script::prelude::*;

/// # Panics
///
pub fn simulation_tick(
    mut script: LoReM<ScriptSimulation>,
    source_map: Re<SourceMapResource>,
    error: Re<ErrorResource>,
) {
    let lookup: &dyn SourceMapLookup = &source_map.wrapper;
    if error.has_errors {
        return;
    }
    script.tick(Some(lookup)).expect("script.tick() crashed");
}

pub fn input_tick(mut script: LoReM<ScriptSimulation>, gamepad_messages: Msg<GamepadMessage>) {
    for gamepad_message in gamepad_messages.iter_current() {
        script.gamepad(gamepad_message);
    }
}

#[derive(Debug)]
pub struct ScriptSimulationContext {}

#[derive(LocalResource, Debug)]
pub struct ScriptSimulation {
    simulation_value_ref: ValueRef,
    simulation_fn: InternalFunctionDefinitionRef,
    gamepad_axis_changed_fn: Option<InternalFunctionDefinitionRef>,
    gamepad_button_changed_fn: Option<InternalFunctionDefinitionRef>,
    external_functions: ExternalFunctions<ScriptSimulationContext>,
    constants: Constants,
    script_context: ScriptSimulationContext,
    resolved_program: Program,
    input_module: ModuleRef,
}

impl ScriptSimulation {
    pub fn new(
        simulation_value_ref: ValueRef,
        simulation_fn: InternalFunctionDefinitionRef,
        gamepad_axis_changed_fn: Option<InternalFunctionDefinitionRef>,
        gamepad_button_changed_fn: Option<InternalFunctionDefinitionRef>,
        external_functions: ExternalFunctions<ScriptSimulationContext>,
        constants: Constants,
        resolved_program: Program,
        input_module: ModuleRef,
    ) -> Self {
        Self {
            simulation_value_ref,
            simulation_fn,
            gamepad_axis_changed_fn,
            gamepad_button_changed_fn,
            external_functions,
            script_context: ScriptSimulationContext {},
            constants,
            resolved_program,
            input_module,
        }
    }

    #[must_use]
    pub fn immutable_simulation_value(&self) -> Value {
        self.simulation_value_ref.borrow().clone()
    }

    pub fn mutable_simulation_value_ref(&mut self) -> &ValueRef {
        &self.simulation_value_ref
    }

    pub fn debug_set_simulation_value(&mut self, value: Value) {
        self.simulation_value_ref = Rc::new(RefCell::new(value));
    }

    /// # Panics
    ///
    #[must_use]
    pub fn main_module(&self) -> ModuleRef {
        let root_module_path = &["crate".to_string(), "simulation".to_string()].to_vec();

        self.resolved_program
            .modules
            .get(root_module_path)
            .expect("simulation module should exist in simulation")
            .clone()
    }

    /// # Errors
    ///
    pub fn tick(
        &mut self,
        debug_source_map: Option<&dyn SourceMapLookup>,
    ) -> Result<(), ExecuteError> {
        let variable_value_ref = VariableValue::Reference(self.simulation_value_ref.clone());
        let _ = util_execute_function(
            &self.external_functions,
            &self.constants,
            &self.simulation_fn,
            &[variable_value_ref],
            &mut self.script_context,
            debug_source_map,
        )?;

        Ok(())
    }

    fn execute(
        &mut self,
        fn_def: &InternalFunctionDefinitionRef,
        arguments: &[Value],
    ) -> Result<(), ExecuteError> {
        let mut complete_arguments = Vec::new();
        complete_arguments.push(VariableValue::Reference(self.simulation_value_ref.clone())); // push simulation self first
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
                self.button_changed(*gamepad_id, *button, *value);
            }
            GamepadMessage::AxisChanged(gamepad_id, axis, value) => {
                self.axis_changed(*gamepad_id, *axis, *value);
            }
        }
    }

    fn axis_changed(&mut self, gamepad_id: GamePadId, axis: Axis, value: AxisValueType) {
        let script_axis_value = {
            let input_module_ref = &self.input_module;
            let axis_str = match axis {
                Axis::LeftStickX => "LeftStickX",
                Axis::LeftStickY => "LeftStickY",
                Axis::RightStickX => "RightStickX",
                Axis::RightStickY => "RightStickY",
            };

            let axis_enum = input_module_ref
                .namespace
                .symbol_table
                .get_enum("Axis")
                .expect("axis")
                .clone();

            let variant = axis_enum
                .borrow()
                .get_variant(axis_str)
                .expect("should be there")
                .clone();

            if let EnumVariantType::Nothing(simple) = &*variant {
                Value::EnumVariantSimple(simple.clone())
            } else {
                panic!("variant axis problem");
            }
        };

        if let Some(found_fn) = &self.gamepad_axis_changed_fn {
            let gamepad_id_value = Value::Int(gamepad_id as i32);
            let axis_value = Value::Float(Fp::from(value));

            let fn_ref = found_fn.clone();

            self.execute(&fn_ref, &[gamepad_id_value, script_axis_value, axis_value])
                .expect("gamepad_axis_changed");
        }
    }

    fn button_changed(&mut self, gamepad_id: GamePadId, button: Button, value: ButtonValueType) {
        let script_button_value = {
            let input_module_ref = &self.input_module;
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

            let button_enum = input_module_ref
                .namespace
                .symbol_table
                .get_enum("Button")
                .expect("button name failed")
                .clone();

            let variant = button_enum
                .borrow()
                .get_variant(button_str)
                .expect("should exist")
                .clone();

            if let EnumVariantType::Nothing(simple) = &*variant {
                Value::EnumVariantSimple(simple.clone())
            } else {
                panic!("variant axis problem");
            }
        };

        if let Some(found_fn) = &self.gamepad_button_changed_fn {
            let gamepad_id_value = Value::Int(
                i32::try_from(gamepad_id).expect("could not convert gamepad button to i32"),
            );
            let button_value = Value::Float(Fp::from(value));

            let fn_ref = found_fn.clone();

            self.execute(
                &fn_ref,
                &[gamepad_id_value, script_button_value, button_value],
            )
            .expect("gamepad_button_changed");
        }
    }
}

/// # Errors
///
pub fn input_module(
    resolve_state: &mut ProgramState,
) -> Result<(SymbolTable, EnumTypeRef, EnumTypeRef), Error> {
    let mut symbol_table = SymbolTable::new();

    let axis_enum_type_ref = {
        let axis_enum_type_id = resolve_state.allocate_number(); // TODO: HACK

        let parent = EnumType {
            name: Node {
                span: Span::default(),
            },
            assigned_name: "Axis".to_string(),
            module_path: Vec::default(),
            type_id: axis_enum_type_id,
            variants: SeqMap::default(),
        };

        let axis_enum_type_ref = symbol_table.add_enum_type(parent)?;

        let variant_names = ["LeftStickX", "LeftStickY", "RightStickX", "RightStickY"];
        let mut resolved_variants = SeqMap::new();
        for (container_index, variant_name) in variant_names.iter().enumerate() {
            let variant_type_id = resolve_state.allocate_number(); // TODO: HACK
            let variant = EnumVariantSimpleType {
                common: EnumVariantCommon {
                    name: Node {
                        span: Span::default(),
                    },
                    assigned_name: variant_name.to_string(),
                    container_index: container_index as u8,
                    number: variant_type_id,
                    owner: axis_enum_type_ref.clone(),
                },
            };

            let complete_variant = EnumVariantType::Nothing(variant.into());

            resolved_variants
                .insert(variant_name.to_string(), Rc::new(complete_variant))
                .expect("works")
        }

        axis_enum_type_ref.borrow_mut().variants = resolved_variants;
        axis_enum_type_ref
    };

    let button_enum_type_ref = {
        let button_enum_type_id = resolve_state.allocate_number(); // TODO: HACK
        // let button_enum_type_id = resolve_state.allocate_number(); // TODO: HACK
        let parent = EnumType {
            name: Node {
                span: Span::default(),
            },
            assigned_name: "Button".to_string(),
            module_path: Vec::default(),
            type_id: button_enum_type_id,
            variants: Default::default(),
        };
        let button_enum_type_ref = symbol_table.add_enum_type(parent)?;

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

        for (container_index, button_variant_name) in button_names.iter().enumerate() {
            let variant_type_id = resolve_state.allocate_number(); // TODO: HACK
            let variant = EnumVariantSimpleType {
                common: EnumVariantCommon {
                    name: Node {
                        span: Span::default(),
                    },
                    assigned_name: button_variant_name.to_string(),
                    container_index: container_index as u8,
                    number: variant_type_id,
                    owner: button_enum_type_ref.clone(),
                },
            };

            let complete_variant = EnumVariantType::Nothing(Rc::new(variant));
            symbol_table.add_enum_variant(button_enum_type_ref.clone(), complete_variant)?;
        }
        button_enum_type_ref
    };

    Ok((symbol_table, axis_enum_type_ref, button_enum_type_ref))
}

/// # Errors
///
/// # Panics
///
pub fn boot(source_map: &mut SourceMapResource) -> Result<ScriptSimulation, MangroveError> {
    let mut resolved_program = Program::new();
    let mut external_functions = ExternalFunctions::<ScriptSimulationContext>::new();

    //let (input_module, _axis_enum_type, _button_enum_type) =
    //  input_module(&mut resolved_program.state)?;
    //let input_module_ref = ModuleRef::from(Module::new(&["input".to_string()], input_module, None));
    // resolved_program.modules.add(input_module_ref.clone());
    let fake_symbol_table = SymbolTable::new();
    let input_module_ref = ModuleRef::from(Module::new(
        &["crate".to_string(), "input".to_string()],
        fake_symbol_table,
        None,
    ));

    let crate_simulation_path = &["crate".to_string(), "simulation".to_string()];

    compile(
        crate_simulation_path,
        &mut resolved_program,
        &mut external_functions,
        &mut source_map.wrapper.source_map,
    )?;

    let main_fn = {
        let main_module = resolved_program
            .modules
            .get(crate_simulation_path)
            .expect("could not find main module");

        let function_ref = main_module
            .namespace
            .symbol_table
            .get_internal_function("main")
            .expect("No main function")
            .clone();

        Rc::clone(&function_ref) // Clone the Rc, not the inner value
    };

    let mut script_context = ScriptSimulationContext {};
    let mut constants = Constants::new();
    eval_constants(
        &external_functions,
        &mut constants,
        &resolved_program.state,
        &mut script_context,
    )?;

    let simulation_value = util_execute_function(
        &external_functions,
        &constants,
        &main_fn,
        &[],
        &mut script_context,
        None,
    )?;

    let Value::NamedStruct(simulation_struct_type_ref, _) = &simulation_value else {
        return Err(MangroveError::Other(
            "needs to be simulation struct".to_string(),
        ));
    };

    let simulation_fn = get_impl_func(
        &resolved_program.state.associated_impls,
        simulation_struct_type_ref,
        "tick",
    );
    let gamepad_axis_changed_fn = get_impl_func_optional(
        &resolved_program.state.associated_impls,
        simulation_struct_type_ref,
        "gamepad_axis_changed",
    );
    let gamepad_button_changed_fn = get_impl_func_optional(
        &resolved_program.state.associated_impls,
        simulation_struct_type_ref,
        "gamepad_button_changed",
    );

    // Convert it to a mutable (reference), so it can be mutated in update ticks
    let simulation_value_ref = Rc::new(RefCell::new(simulation_value));

    Ok(ScriptSimulation::new(
        simulation_value_ref,
        simulation_fn,
        gamepad_axis_changed_fn,
        gamepad_button_changed_fn,
        external_functions,
        constants,
        resolved_program,
        input_module_ref,
    ))
}

pub fn detect_reload_tick(
    script_messages: Msg<ScriptMessage>,
    mut script_simulation: LoReM<ScriptSimulation>,
    mut source_map_resource: ReM<SourceMapResource>,
    mut err: ReM<ErrorResource>,
) {
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => match boot(&mut source_map_resource) {
                Ok(new_simulation) => *script_simulation = new_simulation,
                Err(mangrove_error) => {
                    show_mangrove_error(&mangrove_error, &source_map_resource.wrapper.source_map);
                    err.has_errors = true;

                    //                    eprintln!("script simulation failed: {}", mangrove_error);
                    //                    error!(error=?mangrove_error, "script simulation compile failed");
                }
            },
        }
    }
}

pub struct ScriptSimulationPlugin;

impl Plugin for ScriptSimulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(PreUpdate, detect_reload_tick);
        app.add_system(Update, simulation_tick);
        app.add_system(Update, input_tick);

        // HACK: Just add a completely zeroed out ScriptSimulation and wait for reload message.
        // TODO: Should not try to call updates with params that are not available yet.
        app.insert_local_resource(ScriptSimulation {
            simulation_value_ref: Rc::new(RefCell::new(Value::default())),
            simulation_fn: Rc::new(InternalFunctionDefinition {
                body: Expression {
                    ty: Type::Int,
                    node: Default::default(),
                    kind: ExpressionKind::Break,
                },
                name: LocalIdentifier(Node::default()),
                assigned_name: "".to_string(),
                signature: Signature {
                    parameters: vec![],
                    return_type: Box::from(Type::Int),
                },
            }),
            gamepad_axis_changed_fn: None,
            gamepad_button_changed_fn: None,
            external_functions: ExternalFunctions::new(),
            constants: Constants { values: vec![] },
            script_context: ScriptSimulationContext {},
            resolved_program: Program {
                state: ProgramState {
                    number: 0,
                    external_function_number: 0,
                    constants_in_dependency_order: vec![],
                    associated_impls: Default::default(),
                },
                modules: Modules::default(),
                auto_use_modules: AutoUseModules { modules: vec![] },
            },
            input_module: Rc::new(Module {
                expression: None,
                namespace: Namespace::new(vec![], SymbolTable::default()),
            }),
        });
    }
}
