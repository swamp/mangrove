/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */

use crate::err::show_mangrove_error;
use crate::logic::ScriptLogic;
use crate::script::{
    compile, create_default_color_value, create_default_sprite_params,
    create_empty_struct_value_util, sprite_params, uvec2_like, value_to_value_ref, vec3_like,
    MangroveError,
};
use crate::util::get_impl_func;
use crate::{ErrorResource, ScriptMessage, SourceMapResource};
use monotonic_time_rs::Millis;
use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use swamp::prelude::{
    App, Assets, FixedAtlas, FrameLookup, GameAssets, LoRe, LoReM, LocalResource, MaterialRef, Msg,
    Plugin, Re, ReAll, ReM, Render, ResourceStorage, SpriteParams, UVec2, UpdatePhase, Vec3,
};
use swamp_script::prelude::*;
use tracing::error;

#[derive(Debug)]
pub struct ScriptRenderContext {
    pub(crate) game_assets: Option<GameAssetsWrapper>,
    pub(crate) render: Option<RenderWrapper>,
}

// I didn't want to change the implementation of GameAssets.
// There might be a better way to do this, but I could not find a way.
// Let's do some pointer magic
#[derive(Debug)]
pub struct RenderWrapper {
    render: *mut Render,
}

impl RenderWrapper {
    pub fn new(render: &mut Render) -> Self {
        let ptr = render as *mut Render;
        Self {
            render: ptr as *mut Render, // Coerce. is there a better way?
        }
    }

    pub fn push_sprite(&self, pos: Vec3, material_ref: &MaterialRef) {
        // Safety: We assume the Render pointer is still valid, since the RenderWrapper is short-lived (only alive during a render call)
        let render: &mut Render;
        unsafe {
            render = &mut *self.render;
        }

        render.draw_sprite(pos, material_ref);
    }

    pub fn push_sprite_ex(&self, pos: Vec3, material_ref: &MaterialRef, params: SpriteParams) {
        // Safety: We assume the Render pointer is still valid, since the RenderWrapper is short-lived (only alive during a render call)
        let render: &mut Render;
        unsafe {
            render = &mut *self.render;
        }

        render.draw_sprite_ex(pos, &material_ref, params);
    }

    pub fn sprite_atlas_frame(&mut self, position: Vec3, frame: u16, atlas: &impl FrameLookup) {
        // Safety: We assume the Render pointer is still valid, since the RenderWrapper is short-lived (only alive during a render call)
        let render: &mut Render;
        unsafe {
            render = &mut *self.render;
        }

        render.sprite_atlas_frame(position, frame, atlas);
    }

    pub fn sprite_atlas_frame_ex(
        &mut self,
        position: Vec3,
        frame: u16,
        atlas: &impl FrameLookup,
        params: SpriteParams,
    ) {
        // Safety: We assume the Render pointer is still valid, since the RenderWrapper is short-lived (only alive during a render call)
        let render: &mut Render;
        unsafe {
            render = &mut *self.render;
        }

        render.sprite_atlas_frame_ex(position, frame, atlas, params);
    }
}

// I didn't want to change the implementation of GameAssets.
// There might be a better way to do this, but I could not find a way.
// Let's do some pointer magic
#[derive(Debug)]
pub struct GameAssetsWrapper {
    game_assets: *mut GameAssets<'static>,

    material_struct_type: ResolvedStructTypeRef,
    material_rust_type_ref: ResolvedRustTypeRef,

    fixed_atlas_struct_type_ref: ResolvedStructTypeRef,
    fixed_atlas_rust_type_ref: ResolvedRustTypeRef,
}

#[derive(Debug)]
pub struct FixedAtlasWrapper {
    pub fixed_atlas: FixedAtlas,
}

impl Display for FixedAtlasWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "fixed_atlas {:?} {:?}",
            self.fixed_atlas.one_cell_size, self.fixed_atlas.texture_size
        )
    }
}

#[derive(Clone)]
pub struct MathTypes {
    pub pos2: ResolvedType,
    pub pos2_tuple_type: ResolvedTupleTypeRef,
    pub pos3: ResolvedType,
    pub size2: ResolvedType,
    pub size2_tuple_type: ResolvedTupleTypeRef,
}

pub struct GfxTypes {
    pub color: ResolvedType,
    pub sprite_params: ResolvedType,
}

impl GameAssetsWrapper {
    pub fn new(
        game_assets: &mut GameAssets,
        material_struct_type: ResolvedStructTypeRef,
        material_rust_type_ref: ResolvedRustTypeRef,
        fixed_atlas_struct_type_ref: ResolvedStructTypeRef,
        fixed_atlas_rust_type_ref: ResolvedRustTypeRef,
    ) -> Self {
        let ptr = game_assets as *mut GameAssets;
        Self {
            game_assets: ptr as *mut GameAssets<'static>, // Coerce to 'static. is there a better way?
            material_struct_type,
            material_rust_type_ref,
            fixed_atlas_struct_type_ref,
            fixed_atlas_rust_type_ref,
        }
    }

    fn material_handle(&self, material_ref: MaterialRef) -> Value {
        let material_ref_value = Rc::new(RefCell::new(Value::RustValue(
            self.material_rust_type_ref.clone(),
            Rc::new(RefCell::new(Box::new(material_ref))),
        )));

        Value::Struct(
            self.material_struct_type.clone(),
            [material_ref_value].to_vec(),
        )
    }

    fn fixed_atlas_handle(&self, fixed_atlas: FixedAtlas) -> Value {
        let wrapper = FixedAtlasWrapper { fixed_atlas };
        let fixed_atlas_ref = Rc::new(RefCell::new(Value::RustValue(
            self.fixed_atlas_rust_type_ref.clone(),
            Rc::new(RefCell::new(Box::new(wrapper))),
        )));

        Value::Struct(
            self.fixed_atlas_struct_type_ref.clone(),
            [fixed_atlas_ref.into()].to_vec(),
        )
    }

    pub fn material_png(&self, name: &str) -> Value {
        // Safety: We assume the GameAssets pointer is still valid, since the GameAssetsWrapper is short-lived (only alive during a tick)
        let assets: &mut GameAssets;
        unsafe {
            assets = &mut *self.game_assets;
        }
        let material_ref = assets.material_png(name);

        self.material_handle(material_ref)
    }

    pub fn frame_fixed_grid_material_png(
        &self,
        name: &str,
        grid_size: UVec2,
        texture_size: UVec2,
    ) -> Value {
        // Safety: We assume the GameAssets pointer is still valid, since the GameAssetsWrapper is short-lived (only alive during a tick)
        let assets: &mut GameAssets;
        unsafe {
            assets = &mut *self.game_assets;
        }
        let material_ref = assets.frame_fixed_grid_material_png(name, grid_size, texture_size);

        self.fixed_atlas_handle(material_ref)
    }
}

pub fn register_color_struct_type(
    types: &ResolvedProgramTypes,
    namespace: &mut ResolvedModuleNamespace,
    state: &mut ResolvedProgramState,
    externals: &mut ExternalFunctions<ScriptRenderContext>,
) -> Result<ResolvedStructTypeRef, MangroveError> {
    let mut defined_fields = SeqMap::new();

    let r_field = ResolvedAnonymousStructFieldType {
        identifier: None,
        field_type: types.float_type(),
    };
    defined_fields.insert("r".to_string(), r_field)?;

    let g_field = ResolvedAnonymousStructFieldType {
        identifier: None,
        field_type: types.float_type(),
    };
    defined_fields.insert("g".to_string(), g_field)?;

    let b_field = ResolvedAnonymousStructFieldType {
        identifier: None,
        field_type: types.float_type(),
    };
    defined_fields.insert("b".to_string(), b_field)?;

    let a_field = ResolvedAnonymousStructFieldType {
        identifier: None,
        field_type: types.float_type(),
    };
    defined_fields.insert("a".to_string(), a_field)?;

    let color_type = ResolvedStructType {
        name: Default::default(),
        assigned_name: "Color".to_string(),
        anon_struct_type: ResolvedAnonymousStructType { defined_fields },
        functions: Default::default(),
    };

    let color_struct_type_ref = namespace.add_struct(color_type)?;

    // Color::new()
    let new_external_function_id = state.allocate_external_function_id();
    let new_fn = ResolvedExternalFunctionDefinition {
        name: ResolvedNode::default(),
        signature: FunctionTypeSignature {
            first_parameter_is_self: false,
            parameters: vec![],
            return_type: Box::from(ResolvedType::Struct(color_struct_type_ref.clone())),
        },
        id: new_external_function_id,
    };
    let new_fn_ref = Rc::new(new_fn);
    let new_fn_wrap = ResolvedFunction::External(new_fn_ref.clone());

    let color_type_for_new = color_struct_type_ref.clone();
    externals.register_external_function(
        new_external_function_id,
        move |mem_val: &[VariableValue], _context| {
            let params = convert_to_values(mem_val).expect("should only be values for Color::new");
            let r = params[0].clone();
            let g = params[1].clone();
            let b = params[2].clone();
            let a = params[3].clone();
            Ok(Value::Struct(
                color_type_for_new.clone(),
                value_to_value_ref(&[r, g, b, a].to_vec()),
            ))
        },
    )?;

    // Color::default()
    let default_fn_external_function_id = state.allocate_external_function_id();
    let default_fn = ResolvedExternalFunctionDefinition {
        name: ResolvedNode::default(),
        signature: FunctionTypeSignature {
            first_parameter_is_self: false,
            parameters: vec![],
            return_type: Box::from(ResolvedType::Struct(color_struct_type_ref.clone())),
        },
        id: default_fn_external_function_id,
    };
    let default_fn_ref = Rc::new(default_fn);
    let default_fn_ref_wrap = ResolvedFunction::External(default_fn_ref.clone());

    let color_type_for_default = color_struct_type_ref.clone();
    externals.register_external_function(
        default_fn_external_function_id,
        move |_mem_val: &[VariableValue], _context| {
            Ok(create_default_color_value(color_type_for_default.clone()))
        },
    )?;

    // insert functions
    let mut functions = SeqMap::new();
    functions.insert("new".to_string(), new_fn_wrap.into())?;
    functions.insert("default".to_string(), default_fn_ref_wrap.into())?;

    color_struct_type_ref.borrow_mut().functions = functions;

    Ok(color_struct_type_ref)
}

pub fn register_math_types(types: &ResolvedProgramTypes) -> MathTypes {
    let int_type = types.int_type();

    let position_2_tuple_type = ResolvedTupleType(vec![int_type.clone(), int_type.clone()]);
    let position_2_tuple_type_ref = Rc::new(position_2_tuple_type);
    let pos_2_type_ref = ResolvedType::Tuple(position_2_tuple_type_ref.clone());

    let position_3_tuple_type =
        ResolvedTupleType(vec![int_type.clone(), int_type.clone(), int_type.clone()]);
    let position_tuple_type_ref = Rc::new(position_3_tuple_type);
    let pos_3_type_ref = ResolvedType::Tuple(position_tuple_type_ref);

    let size_int_tuple_type = ResolvedTupleType(vec![int_type.clone(), int_type.clone()]);
    let size_int_tuple_type_ref = Rc::new(size_int_tuple_type);
    let size_2_type_ref = ResolvedType::Tuple(size_int_tuple_type_ref.clone());

    MathTypes {
        pos2: pos_2_type_ref,
        pos2_tuple_type: position_2_tuple_type_ref.clone(),
        pos3: pos_3_type_ref,
        size2: size_2_type_ref,
        size2_tuple_type: size_int_tuple_type_ref.clone(),
    }
}

pub fn register_gfx_types(
    types: &ResolvedProgramTypes,
    state: &mut ResolvedProgramState,
    external_functions: &mut ExternalFunctions<ScriptRenderContext>,
    math_types: &MathTypes,
    namespace: &mut ResolvedModuleNamespace,
) -> Result<GfxTypes, MangroveError> {
    let color_struct_ref =
        register_color_struct_type(&types, namespace, state, external_functions)?;
    let color_type = ResolvedType::Struct(color_struct_ref.clone());
    let sprite_params_struct_ref = register_gfx_sprite_params(
        types,
        state,
        external_functions,
        namespace,
        color_struct_ref,
        math_types,
    )?;
    let sprite_params = ResolvedType::Struct(sprite_params_struct_ref);
    Ok(GfxTypes {
        color: color_type,
        sprite_params,
    })
}

pub fn register_gfx_sprite_params(
    types: &ResolvedProgramTypes,
    state: &mut ResolvedProgramState,
    externals: &mut ExternalFunctions<ScriptRenderContext>,
    namespace: &mut ResolvedModuleNamespace,
    color_type: ResolvedStructTypeRef,
    math_types: &MathTypes,
) -> Result<ResolvedStructTypeRef, MangroveError> {
    // Props
    let mut defined_fields = SeqMap::new();
    let flip_x_field = ResolvedAnonymousStructFieldType {
        identifier: None,
        field_type: types.bool_type(),
    };
    defined_fields.insert("flip_x".to_string(), flip_x_field)?;

    let flip_y_field = ResolvedAnonymousStructFieldType {
        identifier: None,
        field_type: types.bool_type(),
    };
    defined_fields.insert("flip_y".to_string(), flip_y_field)?;

    let rotate_field = ResolvedAnonymousStructFieldType {
        identifier: None,
        field_type: types.int_type(),
    };
    defined_fields.insert("rotate".to_string(), rotate_field)?;

    let color_field = ResolvedAnonymousStructFieldType {
        identifier: None,
        field_type: ResolvedType::Struct(color_type.clone()),
    };
    defined_fields.insert("color".to_string(), color_field)?;

    let scale_field = ResolvedAnonymousStructFieldType {
        identifier: None,
        field_type: types.int_type(),
    };
    defined_fields.insert("scale".to_string(), scale_field)?;

    let uv = ResolvedAnonymousStructFieldType {
        identifier: None,
        field_type: math_types.pos2.clone(),
    };
    defined_fields.insert("uv".to_string(), uv)?;

    let size = ResolvedAnonymousStructFieldType {
        identifier: None,
        field_type: math_types.size2.clone(),
    };
    defined_fields.insert("size".to_string(), size)?;

    let sprite_params_type = ResolvedStructType {
        name: Default::default(),
        assigned_name: "SpriteParams".to_string(),
        anon_struct_type: ResolvedAnonymousStructType { defined_fields },
        functions: Default::default(),
    };
    let sprite_params_struct_type_ref = namespace.add_struct(sprite_params_type)?;

    // SpriteParams::default()
    let default_fn_external_function_id = state.allocate_external_function_id();
    let default_fn = ResolvedExternalFunctionDefinition {
        name: ResolvedNode::default(),
        signature: FunctionTypeSignature {
            first_parameter_is_self: false,
            parameters: vec![],
            return_type: Box::from(ResolvedType::Struct(sprite_params_struct_type_ref.clone())),
        },
        id: default_fn_external_function_id,
    };
    let default_fn_ref = Rc::new(default_fn);
    sprite_params_struct_type_ref
        .borrow_mut()
        .add_external_member_function("default", default_fn_ref)?;

    let sprite_params_struct_ref_for_default = sprite_params_struct_type_ref.clone();
    let color_type_for_default = color_type.clone();
    let math_types_for_default = math_types.clone();
    externals.register_external_function(
        default_fn_external_function_id,
        move |_mem_val: &[VariableValue], _context| {
            Ok(create_default_sprite_params(
                sprite_params_struct_ref_for_default.clone(),
                &color_type_for_default,
                &math_types_for_default.clone(),
            ))
        },
    )?;

    Ok(sprite_params_struct_type_ref)
}

pub fn register_gfx_struct_value_with_members(
    types: &ResolvedProgramTypes,
    mut state: &mut ResolvedProgramState,
    externals: &mut ExternalFunctions<ScriptRenderContext>,
    mut namespace: &mut ResolvedModuleNamespace,
) -> Result<ValueRef, MangroveError> {
    let (gfx_value, gfx_struct_type) = create_empty_struct_value_util(namespace, "Gfx")?;
    let gfx_value_mut = Rc::new(RefCell::new(gfx_value.clone()));
    let assets_general_type = ResolvedType::Struct(gfx_struct_type.clone());

    let math_types = register_math_types(&types);
    let gfx_types = register_gfx_types(&types, &mut state, externals, &math_types, &mut namespace)?;

    let mut_self_parameter = ResolvedTypeForParameter {
        name: "self".to_string(),
        resolved_type: assets_general_type.clone(),
        is_mutable: true,
        node: None,
    };

    let string_type = types.string_type();
    let material_handle = ResolvedTypeForParameter {
        name: "material_handle".to_string(),
        resolved_type: string_type,
        is_mutable: false,
        node: None,
    };

    //position
    let position_param = ResolvedTypeForParameter {
        name: "".to_string(),
        resolved_type: math_types.pos3.clone(),
        is_mutable: false,
        node: None,
    };

    // size
    let size_param = ResolvedTypeForParameter {
        name: "size".to_string(),
        resolved_type: math_types.size2,
        is_mutable: false,
        node: None,
    };

    let sprite_params_parameter = ResolvedTypeForParameter {
        name: "sprite_params".to_string(),
        resolved_type: gfx_types.sprite_params,
        is_mutable: false,
        node: None,
    };

    // sprite() ---------------------------
    let sprite_external_fn_id: ExternalFunctionId = state.allocate_external_function_id();
    let sprite_fn = ResolvedExternalFunctionDefinition {
        name: Default::default(),
        signature: FunctionTypeSignature {
            first_parameter_is_self: true,

            parameters: (&[
                mut_self_parameter.clone(),
                position_param.clone(),
                material_handle.clone(),
                size_param.clone(),
            ])
                .to_vec(),
            return_type: Box::from(types.int_type()),
        },
        id: sprite_external_fn_id,
    };

    let _sprite_fn = gfx_struct_type.borrow_mut().add_external_member_function(
        "sprite",
        ResolvedExternalFunctionDefinitionRef::from(sprite_fn),
    )?;

    externals.register_external_function(
        sprite_external_fn_id,
        move |mem_val: &[VariableValue], context| {
            let params = convert_to_values(mem_val).unwrap();
            //let _self_value = &params[0]; // the Gfx struct is empty by design.
            let position = vec3_like(&params[1])?;

            let material_ref = params[2].downcast_hidden_rust::<MaterialRef>().unwrap();

            context
                .render
                .as_mut()
                .unwrap()
                .push_sprite(position, &material_ref.borrow());

            Ok(Value::Unit)
        },
    )?;

    // sprite_ex() ---------------------------
    let sprite_ex_external_fn_id: ExternalFunctionId = state.allocate_external_function_id();
    let sprite_ex_fn = ResolvedExternalFunctionDefinition {
        name: Default::default(),
        signature: FunctionTypeSignature {
            first_parameter_is_self: true,
            parameters: (&[
                mut_self_parameter.clone(),
                position_param.clone(),
                material_handle.clone(),
                sprite_params_parameter.clone(),
            ])
                .to_vec(),
            return_type: Box::from(types.int_type()),
        },
        id: sprite_ex_external_fn_id,
    };

    let _sprite_ex_fn = gfx_struct_type.borrow_mut().add_external_member_function(
        "sprite_ex",
        ResolvedExternalFunctionDefinitionRef::from(sprite_ex_fn),
    )?;

    externals.register_external_function(
        sprite_ex_external_fn_id,
        move |mem_val: &[VariableValue], context| {
            let params = convert_to_values(mem_val).unwrap();
            //let _self_value = &params[0]; // the Gfx struct is empty by design.
            let position = vec3_like(&params[1])?;

            let material_ref = params[2].downcast_hidden_rust::<MaterialRef>().unwrap();

            let props = sprite_params(&params[3])?;

            context.render.as_mut().unwrap().push_sprite_ex(
                position,
                &material_ref.borrow(),
                props,
            );

            Ok(Value::Unit)
        },
    )?;

    // frame
    let frame = ResolvedTypeForParameter {
        name: Default::default(),
        resolved_type: types.int_type(),
        is_mutable: false,
        node: None,
    };

    // sprite_atlas_frame ---------------------------
    let sprite_atlas_frame_external_fn_id: ExternalFunctionId =
        state.allocate_external_function_id();
    let sprite_atlas_frame_fn = ResolvedExternalFunctionDefinition {
        name: Default::default(),
        signature: FunctionTypeSignature {
            first_parameter_is_self: true,
            parameters: (&[
                mut_self_parameter.clone(),
                position_param.clone(),
                material_handle.clone(),
                frame.clone(),
            ])
                .to_vec(),
            return_type: Box::from(types.int_type()),
        },
        id: sprite_atlas_frame_external_fn_id,
    };
    gfx_struct_type
        .borrow_mut()
        .add_external_member_function("sprite_atlas_frame", sprite_atlas_frame_fn.into())?;

    externals.register_external_function(
        sprite_atlas_frame_external_fn_id,
        move |mem_values: &[VariableValue], context| {
            let params = convert_to_values(mem_values)
                .expect("external function should be given values and no references");
            let position = vec3_like(&params[1])?;

            let material_ref = params[2]
                .downcast_hidden_rust::<FixedAtlasWrapper>()
                .unwrap();

            let frame = &params[3].expect_int()?;

            context.render.as_mut().unwrap().sprite_atlas_frame(
                position,
                frame.abs() as u16,
                &material_ref.as_ref().borrow().fixed_atlas,
            );

            Ok(Value::Unit)
        },
    )?;

    // sprite_atlas_frame_ex ---------------------------

    //--- props

    let sprite_atlas_frame_ex_external_fn_id: ExternalFunctionId =
        state.allocate_external_function_id();
    let sprite_atlas_frame_ex_fn = ResolvedExternalFunctionDefinition {
        name: Default::default(),
        signature: FunctionTypeSignature {
            first_parameter_is_self: true,
            parameters: (&[
                mut_self_parameter,
                position_param,
                material_handle,
                frame,
                sprite_params_parameter,
            ])
                .to_vec(),
            return_type: Box::from(types.int_type()),
        },
        id: sprite_atlas_frame_ex_external_fn_id,
    };
    gfx_struct_type
        .borrow_mut()
        .add_external_member_function("sprite_atlas_frame_ex", sprite_atlas_frame_ex_fn.into())?;

    externals.register_external_function(
        sprite_atlas_frame_ex_external_fn_id,
        move |mem_values: &[VariableValue], context| {
            let params = convert_to_values(mem_values)
                .expect("external function should be given values and no references");
            let position = vec3_like(&params[1])?;

            let material_ref = params[2]
                .downcast_hidden_rust::<FixedAtlasWrapper>()
                .unwrap();

            let frame = &params[3].expect_int()?;

            let props = sprite_params(&params[4])?;

            context.render.as_mut().unwrap().sprite_atlas_frame_ex(
                position,
                frame.abs() as u16,
                &material_ref.as_ref().borrow().fixed_atlas,
                props,
            );

            Ok(Value::Unit)
        },
    )?;

    Ok(gfx_value_mut)
}

pub fn register_asset_struct_value_with_members(
    types: &ResolvedProgramTypes,
    state: &mut ResolvedProgramState,
    externals: &mut ExternalFunctions<ScriptRenderContext>,
    namespace: &mut ResolvedModuleNamespace,
    material_struct_type_ref: ResolvedStructTypeRef,
    fixed_grid_struct_type_ref: ResolvedStructTypeRef,
) -> Result<VariableValue, MangroveError> {
    let (assets_value, assets_type) = create_empty_struct_value_util(namespace, "Assets")?;
    let assets_value_mut =
        VariableValue::Reference(ValueReference(Rc::new(RefCell::new(assets_value.clone()))));

    let assets_general_type = ResolvedType::Struct(assets_type.clone());

    let mut_self_parameter = ResolvedTypeForParameter {
        name: "self".to_string(),
        resolved_type: assets_general_type.clone(),
        is_mutable: true,
        node: None,
    };

    let string_type = types.string_type();
    let asset_name_parameter = ResolvedTypeForParameter {
        name: "asset_name".to_string(),
        resolved_type: string_type,
        is_mutable: false,
        node: None,
    };

    let material_png_function_id: ExternalFunctionId = state.allocate_external_function_id();
    let material_png_def = ResolvedExternalFunctionDefinition {
        name: Default::default(),
        signature: FunctionTypeSignature {
            first_parameter_is_self: false,
            parameters: (&[mut_self_parameter.clone(), asset_name_parameter.clone()]).to_vec(),
            return_type: Box::from(ResolvedType::Struct(material_struct_type_ref)),
        },
        id: material_png_function_id,
    };

    let _material_png_fn = assets_type
        .borrow_mut()
        .add_external_member_function("material_png", material_png_def.into())?;

    externals.register_external_function(
        material_png_function_id,
        move |mem_values: &[VariableValue], context| {
            //let self_value = &params[0]; // Assets is, by design, an empty struct
            let params = convert_to_values(mem_values)
                .expect("should only be passed values to material png function");
            let asset_name = &params[1].expect_string()?;

            Ok(context
                .game_assets
                .as_mut()
                .unwrap()
                .material_png(asset_name))
        },
    )?;

    let int_type = types.int_type();
    let size_int_tuple_type = ResolvedTupleType(vec![int_type.clone(), int_type.clone()]);
    let size_int_tuple_type_ref = Rc::new(size_int_tuple_type);

    let size_param = ResolvedTypeForParameter {
        name: Default::default(),
        resolved_type: ResolvedType::Tuple(size_int_tuple_type_ref),
        is_mutable: false,
        node: None,
    };

    /*
        let _material_png_fn = namespace.util_add_member_external_function(
            &assets_general_type,

        )?;
    */

    let frame_fixed_grid_material_png_function_id: ExternalFunctionId =
        state.allocate_external_function_id();

    let frame_fixed_grid_material_png_def = ResolvedExternalFunctionDefinition {
        name: Default::default(),
        signature: FunctionTypeSignature {
            first_parameter_is_self: false,
            parameters: (&[
                mut_self_parameter,
                asset_name_parameter,
                size_param.clone(),
                size_param,
            ])
                .to_vec(),
            return_type: Box::from(ResolvedType::Struct(fixed_grid_struct_type_ref)),
        },
        id: frame_fixed_grid_material_png_function_id,
    };

    let _frame_fixed_grid_material_png_fn = assets_type.borrow_mut().add_external_member_function(
        "frame_fixed_grid_material_png",
        frame_fixed_grid_material_png_def.into(),
    )?;

    externals.register_external_function(
        frame_fixed_grid_material_png_function_id,
        move |mem_values: &[VariableValue], context| {
            let params = convert_to_values(mem_values)
                .expect("should work to get only values to gfx functions");
            //let self_value = &params[0]; // Assets is, by design, an empty struct
            let asset_name = &params[1].expect_string()?;
            let grid_size = uvec2_like(&params[2])?;
            let texture_size = uvec2_like(&params[3])?; // TODO: Remove this parameter

            Ok(context
                .game_assets
                .as_mut()
                .unwrap()
                .frame_fixed_grid_material_png(asset_name, grid_size, texture_size))
        },
    )?;

    Ok(assets_value_mut)
}

#[derive(LocalResource, Debug)]
pub struct ScriptRender {
    render_value_ref: ValueRef,
    render_fn: ResolvedInternalFunctionDefinitionRef,
    externals: ExternalFunctions<ScriptRenderContext>,
    constants: Constants,
    gfx_struct_ref: ValueRef,
}

impl ScriptRender {
    pub fn new(
        //resource_storage: &mut ResourceStorage,
        render_value_ref: ValueRef,
        render_struct_type_ref: ResolvedStructTypeRef,
        externals: ExternalFunctions<ScriptRenderContext>,
        constants: Constants,
        gfx_struct_ref: ValueRef,
    ) -> Result<Self, MangroveError> {
        let render_fn = get_impl_func(&render_struct_type_ref, "render");

        Ok(Self {
            render_fn,
            render_value_ref,
            gfx_struct_ref,
            externals,
            constants,
        })
    }

    pub fn render(
        &mut self,
        wgpu_render: &mut Render,
        logic_value_ref: &Value,
    ) -> Result<(), ExecuteError> {
        let mut script_context = ScriptRenderContext {
            game_assets: None,
            render: Some(RenderWrapper::new(wgpu_render)),
        };

        let self_mut_ref = VariableValue::Reference(ValueReference(self.render_value_ref.clone()));

        util_execute_function(
            &self.externals,
            &self.constants,
            &self.render_fn,
            &[
                self_mut_ref, //   self.render_value_ref.clone()
                VariableValue::Value(logic_value_ref.clone()),
                VariableValue::Reference(ValueReference(self.gfx_struct_ref.clone())),
            ]
            .to_vec(),
            &mut script_context,
            None,
        )?;

        Ok(())
    }
}

pub fn create_render_module(
    resolved_program: &mut ResolvedProgram,
    mut externals: &mut ExternalFunctions<ScriptRenderContext>,
) -> Result<
    (
        ResolvedModule,
        VariableValue,
        ValueRef,
        ResolvedStructTypeRef,
        ResolvedRustTypeRef,
        ResolvedStructTypeRef,
        ResolvedRustTypeRef,
    ),
    MangroveError,
> {
    let mangrove_render_module =
        ResolvedModule::new(&["mangrove".to_string(), "render".to_string()]);

    let material_handle_rust_type_ref = Rc::new(ResolvedRustType {
        type_name: "MaterialHandle".to_string(),
        number: 91,
    });

    let material_handle_struct_ref = mangrove_render_module
        .namespace
        .borrow_mut()
        .add_generated_struct(
            "MaterialHandle",
            &[(
                "hidden",
                ResolvedType::RustType(material_handle_rust_type_ref.clone()),
            )],
        )?;

    let fixed_atlas_handle_rust_type_ref = Rc::new(ResolvedRustType {
        type_name: "FixedAtlasHandle".to_string(),
        number: 92,
    });

    let fixed_atlas_handle_struct_ref = mangrove_render_module
        .namespace
        .borrow_mut()
        .add_generated_struct(
            "FixedAtlasHandle",
            &[(
                "hidden",
                ResolvedType::RustType(fixed_atlas_handle_rust_type_ref.clone()),
            )],
        )?;

    let assets_struct_value = register_asset_struct_value_with_members(
        &resolved_program.types,
        &mut resolved_program.state,
        &mut externals,
        &mut mangrove_render_module.namespace.borrow_mut(),
        material_handle_struct_ref.clone(),
        fixed_atlas_handle_struct_ref.clone(),
    )?;

    let gfx_struct_value = register_gfx_struct_value_with_members(
        &resolved_program.types,
        &mut resolved_program.state,
        &mut externals,
        &mut mangrove_render_module.namespace.borrow_mut(),
    )?;

    Ok((
        mangrove_render_module,
        assets_struct_value,
        gfx_struct_value,
        material_handle_struct_ref,
        material_handle_rust_type_ref,
        fixed_atlas_handle_struct_ref,
        fixed_atlas_handle_rust_type_ref,
    ))
}

pub fn boot(
    mut resource_storage: &mut ResourceStorage,
    logic_main_module: &ResolvedModuleRef,
) -> Result<ScriptRender, MangroveError> {
    let mut resolved_program = ResolvedProgram::new();
    let mut external_functions = ExternalFunctions::<ScriptRenderContext>::new();

    let (
        render_module,
        assets_value,
        gfx_value,
        material_handle_struct_ref,
        material_handle_rust_type_ref,
        fixed_atlas_struct_type_ref,
        fixed_atlas_handle_rust_type_ref,
    ) = create_render_module(&mut resolved_program, &mut external_functions)?;

    let render_module_ref = Rc::new(RefCell::new(render_module));
    resolved_program.modules.add(render_module_ref);
    resolved_program.modules.add(logic_main_module.clone());

    {
        let source_map = resource_storage.fetch_mut::<SourceMapResource>();
        let base_path = { source_map.base_path().to_path_buf() };
        compile(
            base_path.as_path(),
            "render.swamp".as_ref(),
            &["render".to_string()],
            &mut resolved_program,
            &mut external_functions,
            &mut source_map.wrapper.source_map,
            "render",
        )?;
    }

    let root_module_path = &["render".to_string()].to_vec();
    let main_module = resolved_program
        .modules
        .get(&root_module_path)
        .expect("could not find main module");

    let binding = main_module.borrow();
    let namespace_binding = binding.namespace.borrow_mut();
    let main_fn = namespace_binding
        .get_internal_function("main")
        .expect("No main function");

    let mut game_assets = GameAssets::new(&mut resource_storage, Millis::new(0));

    let mut script_context = ScriptRenderContext {
        game_assets: Some(GameAssetsWrapper::new(
            &mut game_assets,
            material_handle_struct_ref.clone(),
            material_handle_rust_type_ref,
            fixed_atlas_struct_type_ref.clone(),
            fixed_atlas_handle_rust_type_ref,
        )),
        render: None,
    };

    resolved_program.modules.finalize()?;
    let mut constants = Constants::new();
    eval_constants(
        &external_functions,
        &mut constants,
        &resolved_program.modules,
        &mut script_context,
    )?;

    let render_struct_value = util_execute_function(
        &external_functions,
        &constants,
        &main_fn,
        &[assets_value],
        &mut script_context,
        None,
    )?;

    let render_struct_type_ref =
        if let Value::Struct(struct_type_ref, _) = render_struct_value.clone() {
            struct_type_ref
        } else {
            return Err(MangroveError::Other("needs to be logic struct".to_string()));
        };

    // let render_fn = get_impl_func(&render_struct_type_ref, "render");

    // Convert it to a mutable (reference), so it can be mutated in update ticks
    let render_struct_value_mutable_ref = Rc::new(RefCell::new(render_struct_value));

    ScriptRender::new(
        render_struct_value_mutable_ref.clone(),
        render_struct_type_ref.clone(),
        external_functions,
        constants,
        gfx_value,
    )
}

pub fn render_tick(
    mut script: LoReM<ScriptRender>,
    logic: LoRe<ScriptLogic>,
    mut wgpu_render: ReM<Render>,
    error: Re<ErrorResource>,
) {
    if error.has_errors {
        return;
    }
    script
        .render(&mut wgpu_render, &logic.immutable_logic_value())
        .expect("script.render() crashed");
}

pub fn detect_reload_tick(
    script_messages: Msg<ScriptMessage>,
    mut script_render: LoReM<ScriptRender>,
    script_logic: LoRe<ScriptLogic>, // it is important that reload is done in correct order, so the new logic should exist
    source_map: Re<SourceMapResource>,
    mut all_resources: ReAll,
    mut err: ReM<ErrorResource>,
) {
    if err.has_errors {
        return;
    }
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => match boot(&mut all_resources, &script_logic.main_module()) {
                Ok(new_render) => *script_render = new_render,
                Err(mangrove_error) => {
                    err.has_errors = true;
                    show_mangrove_error(&mangrove_error, &source_map.wrapper.source_map);
                    eprintln!("script render failed: {}", mangrove_error);
                    error!(error=?mangrove_error, "script render failed");
                }
            },
        }
    }
}

pub struct ScriptRenderPlugin;

impl Plugin for ScriptRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(UpdatePhase::Update, detect_reload_tick);
        app.add_system(UpdatePhase::Update, render_tick);
        // HACK: Just add a completely zeroed out ScriptRender and wait for reload message.
        // TODO: Should not try to call updates with params that are not available yet.
        app.insert_local_resource(ScriptRender {
            render_value_ref: Rc::new(RefCell::new(Default::default())),
            render_fn: Rc::new(ResolvedInternalFunctionDefinition {
                body: ResolvedExpression::Break(Default::default()),
                name: ResolvedLocalIdentifier(Default::default()),
                signature: FunctionTypeSignature {
                    first_parameter_is_self: false,
                    parameters: vec![],
                    return_type: Box::from(ResolvedType::Any),
                },
                constants: vec![],
            }),
            externals: ExternalFunctions::new(),
            constants: Constants { values: vec![] },
            gfx_struct_ref: Rc::new(RefCell::new(Default::default())),
        });
    }
}
