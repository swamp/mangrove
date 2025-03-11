/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::err::show_mangrove_error;
use crate::main::ScriptMain;
use crate::script::{MangroveError, register_print};
use crate::util::{get_impl_func, get_impl_func_optional};
use crate::{ErrorResource, ScriptMessage, SourceMapResource};
use limnus_gamepad::{Axis, AxisValueType, Button, ButtonValueType, GamePadId, GamepadMessage};
use std::cell::RefCell;
use std::rc::Rc;
use swamp::prelude::{
    App, Fp, LoRe, LoReM, LocalResource, Msg, Plugin, PreUpdate, Re, ReM, Update,
};
use swamp_script::prelude::*;
use tracing::{debug, info};

/// # Panics
///
pub fn simulation_tick(
    mut main: LoReM<ScriptMain>,
    mut script_simulation: LoReM<ScriptSimulation>,
    source_map: Re<SourceMapResource>,
    error: Re<ErrorResource>,
) {
    let lookup: &dyn SourceMapLookup = &source_map.wrapper();
    if error.has_errors {
        return;
    }

    let variable_value_ref =
        VariableValue::Reference(script_simulation.simulation_value_ref.clone());

    let mut script_context = ScriptSimulationContext {};

    let _ = util_execute_function(
        &script_simulation.external_functions,
        &main.constants,
        &script_simulation.simulation_tick_fn,
        &[variable_value_ref],
        &mut script_context,
        Some(lookup),
    )
    .unwrap();
}

pub fn input_tick(
    mut script: LoReM<ScriptSimulation>,
    main: LoRe<ScriptMain>,
    gamepad_messages: Msg<GamepadMessage>,
) {
    for gamepad_message in gamepad_messages.iter_current() {
        script.gamepad(&main, gamepad_message);
    }
}

#[derive(Debug)]
pub struct ScriptSimulationContext {}

#[derive(LocalResource, Debug)]
pub struct ScriptSimulation {
    simulation_value_ref: ValueRef,
    simulation_tick_fn: InternalFunctionDefinitionRef,
    gamepad_axis_changed_fn: Option<InternalFunctionDefinitionRef>,
    gamepad_button_changed_fn: Option<InternalFunctionDefinitionRef>,
    external_functions: ExternalFunctions<ScriptSimulationContext>, // It is empty, but stored for convenience
    script_context: ScriptSimulationContext, // It is empty, but stored for convenience
    input_module: ModuleRef,
}

impl ScriptSimulation {
    pub const fn new(
        simulation_value_ref: ValueRef,
        simulation_fn: InternalFunctionDefinitionRef,
        gamepad_axis_changed_fn: Option<InternalFunctionDefinitionRef>,
        gamepad_button_changed_fn: Option<InternalFunctionDefinitionRef>,
        external_functions: ExternalFunctions<ScriptSimulationContext>,
        input_module: ModuleRef,
    ) -> Self {
        Self {
            simulation_value_ref,
            simulation_tick_fn: simulation_fn,
            gamepad_axis_changed_fn,
            gamepad_button_changed_fn,
            external_functions,
            script_context: ScriptSimulationContext {},
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

    fn execute(
        &mut self,
        script_main: &ScriptMain,
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
            &script_main.constants,
            fn_def,
            &complete_arguments,
            &mut self.script_context,
            None,
        )?;

        Ok(())
    }

    pub fn gamepad(&mut self, script_main: &ScriptMain, msg: &GamepadMessage) {
        match msg {
            GamepadMessage::Connected(_, _) => {}
            GamepadMessage::Disconnected(_) => {}
            GamepadMessage::Activated(_) => {}
            GamepadMessage::ButtonChanged(gamepad_id, button, value) => {
                self.button_changed(script_main, *gamepad_id, *button, *value);
            }
            GamepadMessage::AxisChanged(gamepad_id, axis, value) => {
                self.axis_changed(script_main, *gamepad_id, *axis, *value);
            }
        }
    }

    fn axis_changed(
        &mut self,
        script_main: &ScriptMain,
        gamepad_id: GamePadId,
        axis: Axis,
        value: AxisValueType,
    ) {
        let script_axis_value = {
            let input_module_ref = &self.input_module;
            let axis_str = match axis {
                Axis::LeftStickX => "LeftStickX",
                Axis::LeftStickY => "LeftStickY",
                Axis::RightStickX => "RightStickX",
                Axis::RightStickY => "RightStickY",
            };

            let axis_enum = input_module_ref
                .symbol_table
                .get_enum("Axis")
                .expect("axis")
                .clone();

            let variant = axis_enum
                .get_variant(axis_str)
                .expect("should be there")
                .clone();

            if let EnumVariantType::Nothing(simple) = variant {
                Value::EnumVariantSimple(simple.clone())
            } else {
                panic!("variant axis problem");
            }
        };

        if let Some(found_fn) = &self.gamepad_axis_changed_fn {
            let gamepad_id_value = Value::Int(gamepad_id as i32);
            let axis_value = Value::Float(Fp::from(value));

            let fn_ref = found_fn.clone();

            self.execute(
                script_main,
                &fn_ref,
                &[gamepad_id_value, script_axis_value, axis_value],
            )
            .expect("gamepad_axis_changed");
        }
    }

    fn button_changed(
        &mut self,
        script_main: &ScriptMain,
        gamepad_id: GamePadId,
        button: Button,
        value: ButtonValueType,
    ) {
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
                .symbol_table
                .get_enum("Button")
                .expect("button name failed")
                .clone();

            let variant = button_enum
                .get_variant(button_str)
                .expect("should exist")
                .clone();

            if let EnumVariantType::Nothing(simple) = variant {
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
                script_main,
                &fn_ref,
                &[gamepad_id_value, script_button_value, button_value],
            )
            .expect("gamepad_button_changed");
        }
    }
}

/// # Errors
///
pub fn input_module() -> Result<(SymbolTable, EnumType, EnumType), Error> {
    let mut symbol_table = SymbolTable::new(&["mangrove".to_string(), "input".to_string()]);

    let axis_enum_type_ref = {
        let mut parent = EnumType {
            name: Node {
                span: Span::default(),
            },
            assigned_name: "Axis".to_string(),
            module_path: Vec::default(),
            variants: SeqMap::default(),
        };

        let variant_names = ["LeftStickX", "LeftStickY", "RightStickX", "RightStickY"];
        let mut resolved_variants = SeqMap::new();
        for (container_index, variant_name) in variant_names.iter().enumerate() {
            let variant = EnumVariantSimpleType {
                common: EnumVariantCommon {
                    name: Node {
                        span: Span::default(),
                    },
                    assigned_name: variant_name.to_string(),
                    container_index: container_index as u8,
                },
            };

            let complete_variant = EnumVariantType::Nothing(variant.into());

            resolved_variants
                .insert(variant_name.to_string(), complete_variant)
                .expect("works")
        }

        parent.variants = resolved_variants;

        symbol_table.add_enum_type(parent.clone())?;

        parent
    };

    let button_enum_type_ref = {
        let mut parent = EnumType {
            name: Node {
                span: Span::default(),
            },
            assigned_name: "Button".to_string(),
            module_path: Vec::default(),
            variants: Default::default(),
        };

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
            let variant = EnumVariantSimpleType {
                common: EnumVariantCommon {
                    name: Node {
                        span: Span::default(),
                    },
                    assigned_name: button_variant_name.to_string(),
                    container_index: container_index as u8,
                },
            };

            let complete_variant = EnumVariantType::Nothing(variant);
            // symbol_table.add_enum_variant(&mut button_enum_type_ref, complete_variant)?;
            parent
                .variants
                .insert(button_variant_name.to_string(), complete_variant)
                .unwrap();
        }

        symbol_table.add_enum_type(parent.clone())?;

        parent.clone()
    };

    Ok((symbol_table, axis_enum_type_ref, button_enum_type_ref))
}

pub fn detect_reload_tick(
    script_messages: Msg<ScriptMessage>,
    mut script_simulation: LoReM<ScriptSimulation>,
    script_game: LoRe<ScriptMain>,
    mut source_map_resource: ReM<SourceMapResource>,
    mut err: ReM<ErrorResource>,
) {
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => match boot(&script_game) {
                Ok(new_simulation) => *script_simulation = new_simulation,
                Err(mangrove_error) => {
                    show_mangrove_error(&mangrove_error, &source_map_resource.source_map);
                    err.has_errors = true;

                    //                    eprintln!("script simulation failed: {}", mangrove_error);
                    //                    error!(error=?mangrove_error, "script simulation compile failed");
                }
            },
        }
    }
}

fn boot(script_main: &ScriptMain) -> Result<ScriptSimulation, MangroveError> {
    debug!("boot simulation");
    let mut script_context = ScriptSimulationContext {};

    let mut simulation_externals = ExternalFunctions::<ScriptSimulationContext>::new();

    register_print(
        &script_main.resolved_program.modules,
        &mut simulation_externals,
    );

    let simulation_value = util_execute_function(
        &simulation_externals,
        &script_main.constants,
        &script_main.simulation_new_fn,
        &[],
        &mut script_context,
        None,
    )?;

    let Value::NamedStruct(simulation_struct_type_ref, _) = &simulation_value else {
        return Err(MangroveError::Other(
            "needs to be simulation struct".to_string(),
        ));
    };

    let simulation_tick_fn = get_impl_func(
        &script_main.resolved_program.state.associated_impls,
        simulation_struct_type_ref,
        "tick",
    );
    let gamepad_axis_changed_fn = get_impl_func_optional(
        &script_main.resolved_program.state.associated_impls,
        simulation_struct_type_ref,
        "gamepad_axis_changed",
    );
    let gamepad_button_changed_fn = get_impl_func_optional(
        &script_main.resolved_program.state.associated_impls,
        simulation_struct_type_ref,
        "gamepad_button_changed",
    );

    // Convert it to a mutable (reference), so it can be mutated in update ticks
    let simulation_value_ref = Rc::new(RefCell::new(simulation_value));

    let input_module = input_module()?;

    Ok(ScriptSimulation::new(
        simulation_value_ref,
        simulation_tick_fn,
        gamepad_axis_changed_fn,
        gamepad_button_changed_fn,
        simulation_externals,
        ModuleRef::new(Module::new(SymbolTable::new(&[]), None)),
    ))
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
            simulation_tick_fn: Rc::new(InternalFunctionDefinition {
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
            script_context: ScriptSimulationContext {},
            input_module: Rc::new(Module {
                expression: None,
                symbol_table: SymbolTable::new(&[]),
            }),
        });
    }
}
