/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::err::show_mangrove_error;
use crate::script::MangroveError;
use crate::{util, SourceMapResource};
use limnus_input_binding::{ActionSets, Actions, AnalogAction, DigitalAction, InputConfig};
use swamp::prelude::{App, LocalResource, Plugin};
use swamp_script::prelude::*;
use tracing::info;

#[derive(Debug)]
pub struct ScriptInputContext {}

#[derive(Debug)]
pub enum BindingKind {
    Digital,
    Analog,
}

#[derive(Debug)]
pub struct Binding {
    pub name: String,
    pub struct_field_index: usize,
    pub kind: BindingKind,
}

#[derive(Debug)]
pub struct BindingsInSet {
    pub bindings_in_source_order: Vec<Binding>,
}

#[derive(LocalResource, Debug)]
pub struct ScriptInput {
    pub sets: SeqMap<String, BindingsInSet>,
    pub main_module: ResolvedModuleRef,
}

impl ScriptInput {
    pub fn new(main_module: ResolvedModuleRef, sets: SeqMap<String, BindingsInSet>) -> Self {
        Self { sets, main_module }
    }

    /// # Panics
    ///
    #[must_use]
    pub fn main_module(&self) -> ResolvedModuleRef {
        self.main_module.clone()
    }
}

fn scan_struct(struct_type: &ResolvedStructTypeRef) -> Result<BindingsInSet, MangroveError> {
    let mut bindings_in_source_order = Vec::new();
    for (index, (field_name, field_type)) in struct_type
        .borrow()
        .anon_struct_type
        .defined_fields
        .iter()
        .enumerate()
    {
        info!(ty=?field_type.field_type, "found_field");
        let binding_kind = match &field_type.field_type {
            ResolvedType::Bool => BindingKind::Digital,

            ResolvedType::Tuple(tuple_type) => {
                if tuple_type.0.len() != 2 {
                    return Err(MangroveError::Other("strange field type".into()));
                }
                if tuple_type.0[0] != ResolvedType::Float && tuple_type.0[1] != ResolvedType::Float
                {
                    return Err(MangroveError::Other("strange field type tuple".into()));
                }
                BindingKind::Analog
            }
            _ => {
                return Err(MangroveError::Other("strange field type".into()));
            }
        };

        let binding = Binding {
            name: field_name.clone(),
            struct_field_index: index,
            kind: binding_kind,
        };

        bindings_in_source_order.push(binding);
    }

    let bindings_in_set = BindingsInSet {
        bindings_in_source_order,
    };
    Ok(bindings_in_set)
}

/// # Errors
///
/// # Panics
///
pub fn boot(source_map: &mut SourceMapResource) -> Result<ScriptInput, MangroveError> {
    let input_types = util::compile_types::<i32>(vec![], &["input".to_string()], source_map)?;
    let mut mapping = SeqMap::new();
    info!(len=?input_types.borrow().definitions.len(), "definitions");
    for (name, struct_type) in input_types.borrow().namespace.borrow().structs() {
        let bindings_in_set = scan_struct(struct_type)?;

        mapping.insert(name.clone(), bindings_in_set)?;
    }

    let script_input = ScriptInput {
        sets: mapping,
        main_module: input_types,
    };

    Ok(script_input)
}

#[must_use]
pub fn convert_set_name(s: &str) -> String {
    s.to_lowercase()
}

#[must_use]
pub fn convert_bind_name(s: &str) -> String {
    s.to_lowercase()
}

pub fn convert_to_input_bindings(sets: &SeqMap<String, BindingsInSet>) -> InputConfig {
    let mut all_sets = SeqMap::new();

    for (script_set_name, script_bindings) in sets {
        let converted_set_name = convert_set_name(script_set_name);
        let mut digital_actions = Vec::new();
        let mut analog_actions = Vec::new();
        for script_binding in &script_bindings.bindings_in_source_order {
            let converted_name = convert_bind_name(&script_binding.name);
            match script_binding.kind {
                BindingKind::Digital => digital_actions.push(DigitalAction {
                    name: converted_name,
                }),
                BindingKind::Analog => analog_actions.push(AnalogAction {
                    name: converted_name,
                }),
            };
        }

        let actions = Actions {
            digital: digital_actions,
            analog: analog_actions,
        };

        all_sets.insert(converted_set_name, actions).unwrap();
    }

    InputConfig {
        action_sets: ActionSets { sets: all_sets },
    }
}

pub struct ScriptInputPlugin;

impl Plugin for ScriptInputPlugin {
    fn build(&self, app: &mut App) {
        let source_map_resource = app
            .get_resource_mut::<SourceMapResource>()
            .expect("must have source map resource");

        let script_input_result = boot(source_map_resource);
        match script_input_result {
            Err(mangrove_error) => {
                show_mangrove_error(&mangrove_error, &source_map_resource.wrapper.source_map);
            }

            Ok(script_input) => {
                for (name, set) in &script_input.sets {
                    info!(?name, "found set");
                    for binding in &set.bindings_in_source_order {
                        info!(?binding, "found binding");
                    }
                }

                let converted = convert_to_input_bindings(&script_input.sets);
                info!(?converted, "converted");
                app.insert_resource(converted);

                app.insert_local_resource(script_input);
            }
        }
    }
}
