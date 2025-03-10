/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::main::{ScriptContext, ScriptMain};
use crate::{ErrorResource, SourceMapResource};
use limnus_gamepad::{Axis, AxisValueType, Button, ButtonValueType, GamePadId, GamepadMessage};
use std::cell::RefCell;
use std::rc::Rc;
use swamp::prelude::{App, Fp, LoReM, LocalResource, Msg, Plugin, Re, Update};
use swamp_script::prelude::*;

/// # Panics
///
pub fn simulation_tick(
    mut game: LoReM<ScriptMain>,
    mut script_context: LoReM<ScriptContext>,
    source_map: Re<SourceMapResource>,
    error: Re<ErrorResource>,
) {
    let lookup: &dyn SourceMapLookup = &source_map.wrapper();
    if error.has_errors {
        return;
    }

    let variable_value_ref = VariableValue::Reference(game.simulation_value_ref.clone());
    let _ = util_execute_function(
        &game.external_functions,
        &game.constants,
        &game.simulation_fn,
        &[variable_value_ref],
        &mut script_context,
        None,
    )
    .unwrap();
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
) -> Result<(SymbolTable, EnumType, EnumType), Error> {
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

pub struct ScriptSimulationPlugin;

impl Plugin for ScriptSimulationPlugin {
    fn build(&self, app: &mut App) {
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
                    external_function_number: 0,
                    constants_in_dependency_order: vec![],
                    associated_impls: Default::default(),
                    instantiation_cache: InstantiationCache {
                        cache: Default::default(),
                    },
                },
                modules: Modules::default(),
                default_symbol_table: SymbolTable::new(&[]),
            },
            input_module: Rc::new(Module {
                expression: None,
                symbol_table: SymbolTable::new(&[]),
            }),
        });
    }
}
