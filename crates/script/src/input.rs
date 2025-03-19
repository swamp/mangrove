/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::err::show_mangrove_error;
use crate::script::MangroveError;
use crate::{ErrorResource, ScriptMessage, SourceMapResource, util};
use limnus_basic_input::InputMessage;
use limnus_basic_input::prelude::{ButtonState, MouseButton};
use limnus_input_binding::{ActionSets, Actions, AnalogAction, DigitalAction, InputConfig};
use limnus_screen::WindowMessage;
use std::cell::RefCell;
use std::cmp::{max, min};
use std::rc::Rc;
use swamp::prelude::{
    App, FixedUpdate, LoRe, LoReM, LocalResource, Msg, Plugin, PreUpdate, Re, ReM, Render, URect,
    UVec2, Update, WgpuWindow,
};
use swamp_script::prelude::*;

use crate::script_main::ScriptMain;
use crate::simulation::{ScriptSimulation, ScriptSimulationContext};
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
    //    pub main_module: ModuleRef,
    pub input_value: ValueRef,
    pub mouse_cursor_position_index: usize,
    pub mouse_left_button_index: usize,
    pub mouse_right_button_index: usize,
}

impl ScriptInput {
    pub fn new(
        main_module: ModuleRef,
        sets: SeqMap<String, BindingsInSet>,
        input_value: ValueRef,
        mouse_cursor_position_index: usize,
        mouse_left_button_index: usize,
        mouse_right_button_index: usize,
    ) -> Self {
        Self {
            sets,
            //          main_module,
            input_value,
            mouse_cursor_position_index,
            mouse_left_button_index,
            mouse_right_button_index,
        }
    }
}

pub fn detect_reload_tick(
    script_messages: Msg<ScriptMessage>,
    mut script_input: LoReM<ScriptInput>,
    script_game: LoRe<ScriptMain>,
    mut source_map_resource: ReM<SourceMapResource>,
    mut err: ReM<ErrorResource>,
) {
    if err.has_errors {
        return;
    }
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => match boot(&script_game, &source_map_resource) {
                Ok(new_script_input) => *script_input = new_script_input,
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

/*
#[derive(Message, Debug)]
pub enum WindowMessage {
    CursorMoved(UVec2),
    WindowCreated(),
    Resized(UVec2),
}
 */

pub fn absolute_to_virtual_position(
    physical_position: UVec2,
    viewport: URect,
    virtual_surface_size: UVec2,
) -> UVec2 {
    let relative_x = max(
        0,
        min(
            physical_position.x as i64 - viewport.position.x as i64,
            (viewport.size.x - 1) as i64,
        ),
    );

    let relative_y = max(
        0,
        min(
            physical_position.y as i64 - viewport.position.y as i64,
            (viewport.size.y - 1) as i64,
        ),
    );

    let clamped_to_viewport: UVec2 = UVec2::new(relative_x as u16, relative_y as u16);

    let virtual_position_x =
        (clamped_to_viewport.x as u64 * virtual_surface_size.x as u64) / viewport.size.x as u64;

    let virtual_position_y =
        (clamped_to_viewport.y as u64 * virtual_surface_size.y as u64) / viewport.size.y as u64;

    UVec2::new(virtual_position_x as u16, virtual_position_y as u16)
}

pub fn listen_cursor_moved(
    window_messages: Msg<WindowMessage>,
    mut script_input: LoReM<ScriptInput>,
    wgpu_render: Re<Render>,
) {
    for msg in window_messages.iter_previous() {
        if let WindowMessage::CursorMoved(absolute_position) = msg {
            match &*script_input.input_value.borrow_mut() {
                Value::NamedStruct(_, fields) => {
                    let viewport = wgpu_render.viewport();
                    let virtual_surface_size = wgpu_render.virtual_surface_size();

                    let virtual_position = absolute_to_virtual_position(
                        *absolute_position,
                        viewport,
                        virtual_surface_size,
                    );
                    let must_be_tuple_ref =
                        &fields[script_input.mouse_cursor_position_index].borrow_mut();
                    match &**must_be_tuple_ref {
                        Value::Tuple(_, tuple_fields) => {
                            *tuple_fields[0].borrow_mut() =
                                Value::Int(i32::from(virtual_position.x));
                            *tuple_fields[1].borrow_mut() =
                                Value::Int(i32::from(virtual_position.y));
                        }
                        _ => panic!("internal error"),
                    }
                }

                _ => panic!("internal error"),
            }
        }
    }
}

pub fn listen_mouse_button(input_message: Msg<InputMessage>, mut script_input: LoReM<ScriptInput>) {
    for msg in input_message.iter_previous() {
        if let InputMessage::MouseInput(button_state, mouse_button) = msg {
            let button_index_in_struct = match *mouse_button {
                MouseButton::Left => script_input.mouse_left_button_index,
                MouseButton::Right => script_input.mouse_right_button_index,
                _ => return,
            };
            let new_value = match button_state {
                ButtonState::Pressed => true,
                ButtonState::Released => false,
            };
            match &*script_input.input_value.borrow_mut() {
                Value::NamedStruct(_, fields) => {
                    let mut must_be_bool_value_ref = fields[button_index_in_struct].borrow_mut();
                    *must_be_bool_value_ref = Value::Bool(new_value);
                }

                _ => panic!("internal error"),
            }
        }
    }
}

fn scan_struct(struct_type: &NamedStructType) -> Result<BindingsInSet, MangroveError> {
    let mut bindings_in_source_order = Vec::new();
    for (index, (field_name, field_type)) in struct_type
        .anon_struct_type
        .field_name_sorted_fields
        .iter()
        .enumerate()
    {
        info!(ty=?field_type.field_type, "found_field");
        let binding_kind = match &field_type.field_type {
            Type::Bool => BindingKind::Digital,

            Type::Tuple(tuple_type) => {
                if tuple_type.len() != 2 {
                    return Err(MangroveError::Other("strange field type".into()));
                }
                if tuple_type[0] != Type::Float && tuple_type[1] != Type::Float {
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
pub fn boot(
    script_main: &ScriptMain,
    source_map: &SourceMapResource,
) -> Result<ScriptInput, MangroveError> {
    /*
    let mut mapping = SeqMap::new();
    for (name, struct_type) in input_module.namespace.symbol_table.structs() {
        let bindings_in_set = scan_struct(&struct_type)?;

        mapping.insert(name.clone(), bindings_in_set)?;
    }
    */

    let input_type = script_main.input_new_fn.signature.return_type.clone();

    let Type::NamedStruct(named_struct) = &*input_type else {
        panic!("input must be a named struct");
    };

    let anon_struct = &named_struct.anon_struct_type;

    let Some(mouse_cursor_position_field) = anon_struct
        .field_name_sorted_fields
        .get(&"mouse_cursor_position".to_string())
    else {
        panic!("must have mouse_cursor_position");
    };

    let Some(mouse_cursor_position_index) = anon_struct
        .field_name_sorted_fields
        .get_index(&"mouse_cursor_position".to_string())
    else {
        panic!("must have mouse_cursor_position");
    };

    let Some(mouse_left_button_index) = anon_struct
        .field_name_sorted_fields
        .get_index(&"mouse_left_button".to_string())
    else {
        panic!("must have mouse_left_button");
    };

    let Some(mouse_right_button_index) = anon_struct
        .field_name_sorted_fields
        .get_index(&"mouse_right_button".to_string())
    else {
        panic!("must have mouse_left_button");
    };

    let Type::Tuple(found) = &mouse_cursor_position_field.field_type else {
        panic!("must have mouse_cursor_position tuple");
    };

    assert_eq!(found.len(), 2);
    assert_eq!(found[0], Type::Int);
    assert_eq!(found[1], Type::Int);

    let mut script_context = ScriptInputContext {};

    let mut input_externals = ExternalFunctions::<ScriptInputContext>::new();

    let input_value = util_execute_function(
        &input_externals,
        &script_main.constants,
        &script_main.input_new_fn,
        &[],
        &mut script_context,
        None,
    )?;

    let script_input = ScriptInput {
        sets: SeqMap::default(),
        mouse_cursor_position_index,
        mouse_left_button_index,
        mouse_right_button_index,
        //main_module: Default::default(),
        input_value: Rc::new(RefCell::new(input_value)),
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
        app.add_system(PreUpdate, detect_reload_tick);

        app.add_system(Update, listen_cursor_moved);
        app.add_system(Update, listen_mouse_button);

        app.insert_local_resource(ScriptInput {
            sets: SeqMap::default(),
            input_value: Rc::new(RefCell::new(Value::default())),
            mouse_cursor_position_index: 0,
            mouse_left_button_index: 0,
            mouse_right_button_index: 0,
        });
        /*
        let script_main = app
            .local_resources().get::<ScriptMain>()
            .expect("must have script main");

        let source_map_resource = app
            .resources().get::<SourceMapResource>()
            .expect("must have source map resource");

         */
    }
}
