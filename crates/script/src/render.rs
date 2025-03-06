/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::err::show_mangrove_error;
use crate::script::{
    MangroveError, color_like, compile, create_default_color_value, create_default_sprite_params,
    create_empty_struct_value_util, sprite_params, uvec2_like, value_to_value_ref, vec3_like,
};
use crate::simulation::ScriptSimulation;
use crate::util::get_impl_func;
use crate::{ErrorResource, ScriptMessage, SourceMapResource};
use limnus_screen::WindowMessage;
use monotonic_time_rs::Millis;
use std::cell::RefCell;
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;
use swamp::prelude::{
    App, Assets, Color, FixedAtlas, FontAndMaterial, FrameLookup, GameAssets, Gfx, LoRe, LoReM,
    LocalResource, MaterialRef, Msg, MsgM, Plugin, Re, ReAll, ReM, Render, RenderUpdate,
    ResourceStorage, SpriteParams, UVec2, Update, Vec3, ViewportStrategy, VirtualScale, WgpuWindow,
};
use swamp_script::prelude::*;
use tracing::field::display;
use tracing::{debug, error};

#[derive(Debug)]
pub struct DisplaySettings {
    pub scale: u16,
}

impl DisplaySettings {
    pub fn new() -> Self {
        Self { scale: 0 }
    }

    pub fn set_scale(&mut self, scale: u16) {
        self.scale = scale;
    }
}

#[derive(Debug)]
pub struct ScriptRenderContext {
    pub(crate) game_assets: Option<GameAssetsWrapper>,
    pub(crate) render: Option<RenderWrapper>,
    pub display: DisplaySettings,
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

        render.draw_sprite_ex(pos, material_ref, params);
    }

    pub fn push_quad(&self, pos: Vec3, size: UVec2, color: Color) {
        // Safety: We assume the Render pointer is still valid, since the RenderWrapper is short-lived (only alive during a render call)
        let render: &mut Render;
        unsafe {
            render = &mut *self.render;
        }

        render.draw_quad(pos, size, color);
    }

    pub(crate) fn push_nine_slice(
        &self,
        pos: Vec3,
        size: UVec2,
        corner_size: UVec2,
        texture_window_size: UVec2,
        material: &MaterialRef,
        atlas_offset: UVec2,
        color: Color,
    ) {
        // Safety: We assume the Render pointer is still valid, since the RenderWrapper is short-lived (only alive during a render call)
        let render: &mut Render;
        unsafe {
            render = &mut *self.render;
        }

        render.draw_nine_slice(pos, size, corner_size, size, material, atlas_offset, color);
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

    pub fn text_draw(&self, pos: Vec3, str: &str, material_ref: &FontAndMaterial, color: &Color) {
        // Safety: We assume the Render pointer is still valid, since the RenderWrapper is short-lived (only alive during a render call)
        let render: &mut Render;
        unsafe {
            render = &mut *self.render;
        }

        render.text_draw(pos, str, material_ref, color);
    }

    pub fn set_virtual_resolution(&self, virtual_resolution: UVec2) {
        // Safety: We assume the Render pointer is still valid, since the RenderWrapper is short-lived (only alive during a render call)
        let render: &mut Render;
        unsafe {
            render = &mut *self.render;
        }

        render.set_viewport(ViewportStrategy::FitIntegerScaling(virtual_resolution));
    }
}

// I didn't want to change the implementation of GameAssets.
// There might be a better way to do this, but I could not find a way.
// Let's do some pointer magic
#[derive(Debug)]
pub struct GameAssetsWrapper {
    game_assets: *mut GameAssets<'static>,

    material_struct_type: NamedStructTypeRef,
    material_rust_type_ref: ExternalTypeRef,

    fixed_atlas_struct_type_ref: NamedStructTypeRef,
    fixed_atlas_rust_type_ref: ExternalTypeRef,

    font_and_material_struct_type_ref: NamedStructTypeRef,
    font_and_material_rust_type_ref: ExternalTypeRef,
}

#[derive(Debug, PartialEq)]
pub struct FixedAtlasWrapper {
    pub fixed_atlas: FixedAtlas,
}

impl QuickSerialize for FixedAtlasWrapper {}

impl Display for FixedAtlasWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "fixed_atlas {:?} {:?}",
            self.fixed_atlas.one_cell_size, self.fixed_atlas.texture_size
        )
    }
}

#[derive(Debug, PartialEq)]
pub struct FontAndMaterialWrapper {
    pub font_and_material: FontAndMaterial,
}

impl QuickSerialize for FontAndMaterialWrapper {}
impl Display for FontAndMaterialWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "font_and_material {:?} {:?}",
            self.font_and_material.font_ref.asset_name().unwrap(),
            self.font_and_material.material_ref.asset_name().unwrap()
        )
    }
}

#[derive(Clone)]
pub struct MathTypes {
    pub pos2: Type,
    pub pos2_tuple_type: Vec<Type>,
    pub pos3: Type,
    pub size2: Type,
    pub size2_tuple_type: Vec<Type>,
}

pub struct GfxTypes {
    pub color: Type,
    pub sprite_params: Type,
}

#[derive(PartialEq)]
pub struct MaterialWrapper(pub MaterialRef);

impl Display for MaterialWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Debug for MaterialWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}
impl QuickSerialize for MaterialWrapper {}

impl GameAssetsWrapper {
    pub fn new(
        game_assets: &mut GameAssets,
        material_struct_type: NamedStructTypeRef,
        material_rust_type_ref: ExternalTypeRef,
        fixed_atlas_struct_type_ref: NamedStructTypeRef,
        fixed_atlas_rust_type_ref: ExternalTypeRef,

        font_and_material_struct_type_ref: NamedStructTypeRef,
        font_and_material_rust_type_ref: ExternalTypeRef,
    ) -> Self {
        let ptr = game_assets as *mut GameAssets;
        Self {
            game_assets: ptr as *mut GameAssets<'static>, // Coerce to 'static. is there a better way?
            material_struct_type,
            material_rust_type_ref,
            fixed_atlas_struct_type_ref,
            fixed_atlas_rust_type_ref,
            font_and_material_struct_type_ref,
            font_and_material_rust_type_ref,
        }
    }

    fn material_handle(&self, material_ref: MaterialRef) -> Value {
        let material_ref_value = Rc::new(RefCell::new(Value::RustValue(
            self.material_rust_type_ref.clone(),
            Rc::new(RefCell::new(Box::new(MaterialWrapper(material_ref)))),
        )));

        Value::NamedStruct(
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

        Value::NamedStruct(
            self.fixed_atlas_struct_type_ref.clone(),
            [fixed_atlas_ref].to_vec(),
        )
    }

    fn font_and_material_handle(&self, font_and_material: FontAndMaterial) -> Value {
        let wrapper = FontAndMaterialWrapper { font_and_material };
        let font_and_material_ref = Rc::new(RefCell::new(Value::RustValue(
            self.font_and_material_rust_type_ref.clone(),
            Rc::new(RefCell::new(Box::new(wrapper))),
        )));

        Value::NamedStruct(
            self.font_and_material_struct_type_ref.clone(),
            [font_and_material_ref].to_vec(),
        )
    }

    #[must_use]
    pub fn material_png(&self, name: &str) -> Value {
        // Safety: We assume the GameAssets pointer is still valid, since the GameAssetsWrapper is short-lived (only alive during a tick)
        let assets: &mut GameAssets;
        unsafe {
            assets = &mut *self.game_assets;
        }
        let material_ref = assets.material_png(name);

        self.material_handle(material_ref)
    }

    #[must_use]
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

    #[must_use]
    pub fn bm_font(&self, name: &str) -> Value {
        let assets: &mut GameAssets;
        unsafe {
            // Safety: We assume the GameAssets pointer is still valid, since the GameAssetsWrapper is short-lived (only alive during a tick)

            assets = &mut *self.game_assets;
        }
        let font_and_material_ref = assets.bm_font(name);

        self.font_and_material_handle(font_and_material_ref)
    }
}

/// # Errors
///
/// # Panics
///
#[allow(clippy::too_many_lines)]
pub fn register_color_struct_type(
    symbol_table: &mut SymbolTable,
    state: &mut ProgramState,
    externals: &mut ExternalFunctions<ScriptRenderContext>,
) -> Result<NamedStructTypeRef, MangroveError> {
    let mut defined_fields = SeqMap::new();

    let r_field = StructTypeField {
        identifier: None,
        field_type: Type::Float,
    };
    defined_fields.insert("r".to_string(), r_field)?;

    let g_field = StructTypeField {
        identifier: None,
        field_type: Type::Float,
    };
    defined_fields.insert("g".to_string(), g_field)?;

    let b_field = StructTypeField {
        identifier: None,
        field_type: Type::Float,
    };
    defined_fields.insert("b".to_string(), b_field)?;

    let a_field = StructTypeField {
        identifier: None,
        field_type: Type::Float,
    };
    defined_fields.insert("a".to_string(), a_field)?;

    let color_type_id = state.allocate_number();
    let color_type = NamedStructType {
        name: Node::default(),
        assigned_name: "Color".to_string(),
        anon_struct_type: AnonymousStructType::new(defined_fields),
        type_id: color_type_id,
    };

    let color_struct_type_ref = symbol_table.add_struct(color_type)?;
    let color_general_type = Type::NamedStruct(color_struct_type_ref.clone());
    state.associated_impls.prepare(&color_general_type);

    // Color::new(r, g, b, a)
    let new_external_function_id = state.allocate_external_function_id();
    let new_fn = ExternalFunctionDefinition {
        name: None,
        assigned_name: "new".to_string(),
        signature: Signature {
            parameters: vec![
                TypeForParameter {
                    name: "r".to_string(),
                    resolved_type: Type::Float,
                    is_mutable: false,
                    node: None,
                },
                TypeForParameter {
                    name: "g".to_string(),
                    resolved_type: Type::Float,
                    is_mutable: false,
                    node: None,
                },
                TypeForParameter {
                    name: "b".to_string(),
                    resolved_type: Type::Float,
                    is_mutable: false,
                    node: None,
                },
                TypeForParameter {
                    name: "a".to_string(),
                    resolved_type: Type::Float,
                    is_mutable: false,
                    node: None,
                },
            ],
            return_type: Box::from(Type::NamedStruct(color_struct_type_ref.clone())),
        },
        id: new_external_function_id,
    };
    let new_fn_ref = Rc::new(new_fn);
    let new_fn_wrap = Function::External(new_fn_ref);

    let color_type_for_new = color_struct_type_ref.clone();
    externals.register_external_function(
        new_external_function_id,
        move |mem_val: &[VariableValue], _context| {
            let params = convert_to_values(mem_val).expect("should only be values for Color::new");
            let r = params[0].clone();
            let g = params[1].clone();
            let b = params[2].clone();
            let a = params[3].clone();
            Ok(Value::NamedStruct(
                color_type_for_new.clone(),
                value_to_value_ref([r, g, b, a].as_ref()),
            ))
        },
    )?;

    // Color::default()
    let default_fn_external_function_id = state.allocate_external_function_id();
    let default_fn = ExternalFunctionDefinition {
        name: None,
        assigned_name: "default".to_string(),
        signature: Signature {
            parameters: vec![],
            return_type: Box::from(Type::NamedStruct(color_struct_type_ref.clone())),
        },
        id: default_fn_external_function_id,
    };
    let default_fn_ref = Rc::new(default_fn);
    let default_fn_ref_wrap = Function::External(default_fn_ref);

    let color_type_for_default = color_struct_type_ref.clone();
    externals.register_external_function(
        default_fn_external_function_id,
        move |_mem_val: &[VariableValue], _context| {
            Ok(create_default_color_value(color_type_for_default.clone()))
        },
    )?;

    // insert functions

    //functions.insert("new".to_string(), new_fn_wrap.into())?;
    //functions.insert("default".to_string(), default_fn_ref_wrap.into())?;

    state
        .associated_impls
        .add_external_struct_member_function(&color_struct_type_ref, new_fn_wrap)?;

    Ok(color_struct_type_ref)
}

#[must_use]
pub fn register_math_types() -> MathTypes {
    let int_type = Type::Int;

    let position_2_tuple_type = vec![int_type.clone(), int_type.clone()];
    let pos_2_type_ref = Type::Tuple(position_2_tuple_type.clone());

    let position_3_tuple_type = vec![int_type.clone(), int_type.clone(), int_type.clone()];
    let pos_3_type_ref = Type::Tuple(position_3_tuple_type);

    let size_int_tuple_type = vec![int_type.clone(), int_type];
    let size_2_type_ref = Type::Tuple(size_int_tuple_type.clone());

    MathTypes {
        pos2: pos_2_type_ref,
        pos2_tuple_type: position_2_tuple_type,
        pos3: pos_3_type_ref,
        size2: size_2_type_ref,
        size2_tuple_type: size_int_tuple_type,
    }
}

/// # Errors
///
pub fn register_gfx_types(
    state: &mut ProgramState,
    external_functions: &mut ExternalFunctions<ScriptRenderContext>,
    math_types: &MathTypes,
    symbol_table: &mut SymbolTable,
) -> Result<GfxTypes, MangroveError> {
    let color_struct_ref = register_color_struct_type(symbol_table, state, external_functions)?;
    let color_type = Type::NamedStruct(color_struct_ref.clone());
    let sprite_params_struct_ref = register_gfx_sprite_params(
        state,
        external_functions,
        symbol_table,
        color_struct_ref,
        math_types,
    )?;
    let sprite_params = Type::NamedStruct(sprite_params_struct_ref);
    Ok(GfxTypes {
        color: color_type,
        sprite_params,
    })
}

/// # Errors
///
pub fn register_gfx_sprite_params(
    state: &mut ProgramState,
    externals: &mut ExternalFunctions<ScriptRenderContext>,
    symbol_table: &mut SymbolTable,
    color_type: NamedStructTypeRef,
    math_types: &MathTypes,
) -> Result<NamedStructTypeRef, MangroveError> {
    // Props
    let mut defined_fields = SeqMap::new();
    let flip_x_field = StructTypeField {
        identifier: None,
        field_type: Type::Bool,
    };
    defined_fields.insert("flip_x".to_string(), flip_x_field)?;

    let flip_y_field = StructTypeField {
        identifier: None,
        field_type: Type::Bool,
    };
    defined_fields.insert("flip_y".to_string(), flip_y_field)?;

    let rotate_field = StructTypeField {
        identifier: None,
        field_type: Type::Int,
    };
    defined_fields.insert("rotate".to_string(), rotate_field)?;

    let color_field = StructTypeField {
        identifier: None,
        field_type: Type::NamedStruct(color_type.clone()),
    };
    defined_fields.insert("color".to_string(), color_field)?;

    let scale_field = StructTypeField {
        identifier: None,
        field_type: Type::Int,
    };
    defined_fields.insert("scale".to_string(), scale_field)?;

    let uv = StructTypeField {
        identifier: None,
        field_type: math_types.pos2.clone(),
    };
    defined_fields.insert("uv".to_string(), uv)?;

    let size = StructTypeField {
        identifier: None,
        field_type: math_types.size2.clone(),
    };
    defined_fields.insert("size".to_string(), size)?;

    let sprite_params_type_id = state.allocate_number();
    let sprite_params_type = NamedStructType {
        name: Node::default(),
        assigned_name: "SpriteParams".to_string(),
        anon_struct_type: AnonymousStructType::new(defined_fields),
        type_id: sprite_params_type_id,
    };
    let sprite_params_struct_type_ref = symbol_table.add_struct(sprite_params_type)?;
    let sprite_params_general_type = Type::NamedStruct(sprite_params_struct_type_ref.clone());
    state.associated_impls.prepare(&sprite_params_general_type);

    // SpriteParams::default()
    let default_fn_external_function_id = state.allocate_external_function_id();
    let default_fn = ExternalFunctionDefinition {
        name: None,
        assigned_name: "default".to_string(),
        signature: Signature {
            parameters: vec![],
            return_type: Box::from(sprite_params_general_type),
        },
        id: default_fn_external_function_id,
    };
    let default_fn_ref = Rc::new(default_fn);
    state
        .associated_impls
        .add_external_struct_member_function_external_ref(
            sprite_params_struct_type_ref.clone(),
            default_fn_ref,
        )?;

    let sprite_params_struct_ref_for_default = sprite_params_struct_type_ref.clone();
    let color_type_for_default = color_type;
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

/// # Errors
/// # Panics
///
#[allow(clippy::too_many_lines)]
pub fn register_gfx_struct_value_with_members(
    state: &mut ProgramState,
    externals: &mut ExternalFunctions<ScriptRenderContext>,
    symbol_table: &mut SymbolTable,
) -> Result<ValueRef, MangroveError> {
    let gfx_type_id = state.allocate_number();
    let (gfx_value, gfx_struct_type) =
        create_empty_struct_value_util(symbol_table, "Gfx", gfx_type_id)?;
    let gfx_value_mut = Rc::new(RefCell::new(gfx_value));
    let gfx_general_type = Type::NamedStruct(gfx_struct_type.clone());

    state.associated_impls.prepare(&gfx_general_type);

    let math_types = register_math_types();
    let gfx_types = register_gfx_types(state, externals, &math_types, symbol_table)?;

    let mut_self_parameter = TypeForParameter {
        name: "self".to_string(),
        resolved_type: gfx_general_type,
        is_mutable: true,
        node: None,
    };

    let string_type = Type::String;

    let material_handle_type_ref = symbol_table
        .get_struct("MaterialHandle")
        .expect("material handle is missing");

    let material_handle = TypeForParameter {
        name: "material_handle".to_string(),
        resolved_type: Type::NamedStruct(material_handle_type_ref.clone()),
        is_mutable: false,
        node: None,
    };

    let fixed_atlas_handle_type_ref = symbol_table
        .get_struct("FixedAtlasHandle")
        .expect("FixedAtlasHandle is missing");
    let fixed_atlas_handle = TypeForParameter {
        name: "fixed_atlas_handle".to_string(),
        resolved_type: Type::NamedStruct(fixed_atlas_handle_type_ref.clone()),
        is_mutable: false,
        node: None,
    };

    //position
    let position_param = TypeForParameter {
        name: "position".to_string(),
        resolved_type: math_types.pos3.clone(),
        is_mutable: false,
        node: None,
    };

    // size
    let size_param = TypeForParameter {
        name: "size".to_string(),
        resolved_type: math_types.size2,
        is_mutable: false,
        node: None,
    };

    // int
    let int_param = TypeForParameter {
        name: "i".to_string(),
        resolved_type: Type::Int,
        is_mutable: false,
        node: None,
    };

    let sprite_params_parameter = TypeForParameter {
        name: "sprite_params".to_string(),
        resolved_type: gfx_types.sprite_params,
        is_mutable: false,
        node: None,
    };

    // sprite() ---------------------------
    let sprite_external_fn_id: ExternalFunctionId = state.allocate_external_function_id();
    let sprite_fn = ExternalFunctionDefinition {
        name: None,
        assigned_name: "sprite".to_string(),
        signature: Signature {
            parameters: [
                mut_self_parameter.clone(),
                position_param.clone(),
                material_handle.clone(),
                size_param.clone(),
            ]
            .to_vec(),
            return_type: Box::from(Type::Int),
        },
        id: sprite_external_fn_id,
    };

    state
        .associated_impls
        .add_external_struct_member_function_external(gfx_struct_type.clone(), sprite_fn)?;

    externals.register_external_function(
        sprite_external_fn_id,
        move |mem_val: &[VariableValue], context| {
            let params = convert_to_values(mem_val).unwrap();
            //let _self_value = &params[0]; // the Gfx struct is empty by design.
            let position = vec3_like(&params[1])?;

            let material_ref = params[2].downcast_hidden_rust::<MaterialWrapper>().unwrap();

            context
                .render
                .as_mut()
                .unwrap()
                .push_sprite(position, &material_ref.borrow().0);

            Ok(Value::Unit)
        },
    )?;

    // sprite_ex() ---------------------------
    let sprite_ex_external_fn_id: ExternalFunctionId = state.allocate_external_function_id();
    let sprite_ex_fn = ExternalFunctionDefinition {
        name: None,
        assigned_name: "sprite_ex".to_string(),
        signature: Signature {
            parameters: [
                mut_self_parameter.clone(),
                position_param.clone(),
                material_handle.clone(),
                sprite_params_parameter.clone(),
            ]
            .to_vec(),
            return_type: Box::from(Type::Unit),
        },
        id: sprite_ex_external_fn_id,
    };

    state
        .associated_impls
        .add_external_struct_member_function_external(gfx_struct_type.clone(), sprite_ex_fn)?;

    externals.register_external_function(
        sprite_ex_external_fn_id,
        move |mem_val: &[VariableValue], context| {
            let params = convert_to_values(mem_val).unwrap();
            //let _self_value = &params[0]; // the Gfx struct is empty by design.
            let position = vec3_like(&params[1])?;

            let material_ref = params[2].downcast_hidden_rust::<MaterialWrapper>().unwrap();

            let props = sprite_params(&params[3])?;

            context.render.as_mut().unwrap().push_sprite_ex(
                position,
                &material_ref.borrow().0,
                props,
            );

            Ok(Value::Unit)
        },
    )?;

    let color_parameter = TypeForParameter {
        name: "color".to_string(),
        resolved_type: gfx_types.color,
        is_mutable: false,
        node: None,
    };

    // quad() ---------------------------
    let quad_external_fn_id: ExternalFunctionId = state.allocate_external_function_id();
    let quad_fn = ExternalFunctionDefinition {
        name: None,
        assigned_name: "quad".to_string(),
        signature: Signature {
            parameters: [
                mut_self_parameter.clone(),
                position_param.clone(),
                size_param.clone(),
                color_parameter.clone(),
            ]
            .to_vec(),
            return_type: Box::from(Type::Unit),
        },
        id: quad_external_fn_id,
    };

    state
        .associated_impls
        .add_external_struct_member_function_external(gfx_struct_type.clone(), quad_fn)?;

    externals.register_external_function(
        quad_external_fn_id,
        move |mem_val: &[VariableValue], context| {
            let params = convert_to_values(mem_val).unwrap();
            //let _self_value = &params[0]; // the Gfx struct is empty by design.
            let position = vec3_like(&params[1])?;

            let size = uvec2_like(&params[2])?;

            let color = color_like(&params[3])?;

            context
                .render
                .as_mut()
                .unwrap()
                .push_quad(position, size, color);

            Ok(Value::Unit)
        },
    )?;

    // [9-slice](https://en.wikipedia.org/wiki/9-slice_scaling)
    let nine_slice_external_fn_id: ExternalFunctionId = state.allocate_external_function_id();
    let nine_slice_fn = ExternalFunctionDefinition {
        name: None,
        assigned_name: "nine_slice".to_string(),
        signature: Signature {
            parameters: [
                mut_self_parameter.clone(),
                position_param.clone(),
                size_param.clone(),
                size_param.clone(),
                material_handle,
                size_param.clone(),
                color_parameter.clone(),
            ]
            .to_vec(),
            return_type: Box::from(Type::Unit),
        },
        id: nine_slice_external_fn_id,
    };

    state
        .associated_impls
        .add_external_struct_member_function_external(gfx_struct_type.clone(), nine_slice_fn)?;

    externals.register_external_function(
        nine_slice_external_fn_id,
        move |mem_val: &[VariableValue], context| {
            let params = convert_to_values(mem_val).unwrap();
            //let _self_value = &params[0]; // the Gfx struct is empty by design.
            let position = vec3_like(&params[1])?;
            let complete_size = uvec2_like(&params[2])?;
            let corner_size = uvec2_like(&params[3])?;
            let material_ref = params[4].downcast_hidden_rust::<MaterialWrapper>().unwrap();
            let atlas_offset = uvec2_like(&params[5])?;
            let color = color_like(&params[6])?;

            context.render.as_mut().unwrap().push_nine_slice(
                position,
                complete_size,
                corner_size,
                complete_size,
                &material_ref.borrow().0,
                atlas_offset,
                color,
            );

            Ok(Value::Unit)
        },
    )?;

    let font_and_material_handle_ref = symbol_table
        .get_struct("FontAndMaterialHandle")
        .expect("FontAndMaterialHandle is missing");

    let font_and_material_handle = TypeForParameter {
        name: "font_and_material_handle".to_string(),
        resolved_type: Type::NamedStruct(font_and_material_handle_ref.clone()),
        is_mutable: false,
        node: None,
    };

    let text_parameter = TypeForParameter {
        name: "str".to_string(),
        resolved_type: string_type,
        is_mutable: false,
        node: None,
    };

    // text() ---------------------------
    let text_external_fn_id: ExternalFunctionId = state.allocate_external_function_id();
    let text_fn = ExternalFunctionDefinition {
        name: None,
        assigned_name: "text".to_string(),
        signature: Signature {
            parameters: [
                mut_self_parameter.clone(),
                position_param.clone(),
                text_parameter,
                font_and_material_handle,
                color_parameter.clone(),
            ]
            .to_vec(),
            return_type: Box::from(Type::Unit),
        },
        id: text_external_fn_id,
    };

    state
        .associated_impls
        .add_external_struct_member_function_external(gfx_struct_type.clone(), text_fn)?;

    externals.register_external_function(
        text_external_fn_id,
        move |mem_val: &[VariableValue], context| {
            let params = convert_to_values(mem_val).unwrap();
            //let _self_value = &params[0]; // the Gfx struct is empty by design.
            let position = vec3_like(&params[1])?;
            let text = params[2].expect_string()?;

            let font_and_material_wrapper_ref = params[3]
                .downcast_hidden_rust::<FontAndMaterialWrapper>()
                .unwrap();
            let color = color_like(&params[4])?;

            context.render.as_mut().unwrap().text_draw(
                position,
                &text,
                &font_and_material_wrapper_ref.borrow().font_and_material,
                &color,
            );

            Ok(Value::Unit)
        },
    )?;

    // frame
    let frame = TypeForParameter {
        name: String::default(),
        resolved_type: Type::Int,
        is_mutable: false,
        node: None,
    };

    // sprite_atlas_frame ---------------------------
    let sprite_atlas_frame_external_fn_id: ExternalFunctionId =
        state.allocate_external_function_id();
    let sprite_atlas_frame_fn = ExternalFunctionDefinition {
        name: None,
        assigned_name: "sprite_atlas_frame".to_string(),
        signature: Signature {
            parameters: [
                mut_self_parameter.clone(),
                position_param.clone(),
                fixed_atlas_handle.clone(),
                frame.clone(),
            ]
            .to_vec(),
            return_type: Box::from(Type::Unit),
        },
        id: sprite_atlas_frame_external_fn_id,
    };
    state
        .associated_impls
        .add_external_struct_member_function_external(
            gfx_struct_type.clone(),
            sprite_atlas_frame_fn,
        )?;

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
                u16::try_from(frame.abs()).expect("could not convert to u16 sprite frame"),
                &material_ref.as_ref().borrow().fixed_atlas,
            );

            Ok(Value::Unit)
        },
    )?;

    // sprite_atlas_frame_ex ---------------------------

    //--- props

    let sprite_atlas_frame_ex_external_fn_id: ExternalFunctionId =
        state.allocate_external_function_id();
    let sprite_atlas_frame_ex_fn = ExternalFunctionDefinition {
        name: None,
        assigned_name: "sprite_atlas_frame_ex".to_string(),
        signature: Signature {
            parameters: [
                mut_self_parameter.clone(),
                position_param,
                fixed_atlas_handle,
                frame,
                sprite_params_parameter,
            ]
            .to_vec(),
            return_type: Box::from(Type::Unit),
        },
        id: sprite_atlas_frame_ex_external_fn_id,
    };
    state
        .associated_impls
        .add_external_struct_member_function_external(
            gfx_struct_type.clone(),
            sprite_atlas_frame_ex_fn,
        )?;

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
                u16::try_from(frame.abs()).expect("could not get sprite atlas frame"),
                &material_ref.as_ref().borrow().fixed_atlas,
                props,
            );

            Ok(Value::Unit)
        },
    )?;

    // --- set virtual resolution
    let set_virtual_resolution_external_fn_id: ExternalFunctionId =
        state.allocate_external_function_id();
    let set_virtual_resolution_fn = ExternalFunctionDefinition {
        name: None,
        assigned_name: "set_virtual_resolution".to_string(),
        signature: Signature {
            parameters: [mut_self_parameter.clone(), size_param].to_vec(),
            return_type: Box::from(Type::Unit),
        },
        id: set_virtual_resolution_external_fn_id,
    };
    state
        .associated_impls
        .add_external_struct_member_function_external(
            gfx_struct_type.clone(),
            set_virtual_resolution_fn,
        )?;

    externals.register_external_function(
        set_virtual_resolution_external_fn_id,
        move |mem_values: &[VariableValue], context| {
            let params = convert_to_values(mem_values)
                .expect("external function should be given values and no references");
            let size = uvec2_like(&params[1])?;

            context
                .render
                .as_mut()
                .unwrap()
                .set_virtual_resolution(size);

            Ok(Value::Unit)
        },
    )?;

    // --- set resolution scale
    let set_resolution_scale_external_fn_id: ExternalFunctionId =
        state.allocate_external_function_id();
    let set_resolution_scale_fn = ExternalFunctionDefinition {
        name: None,
        assigned_name: "set_resolution_scale".to_string(),
        signature: Signature {
            parameters: [mut_self_parameter.clone(), int_param].to_vec(),
            return_type: Box::from(Type::Unit),
        },
        id: set_resolution_scale_external_fn_id,
    };
    state
        .associated_impls
        .add_external_struct_member_function_external(
            gfx_struct_type.clone(),
            set_resolution_scale_fn,
        )?;

    externals.register_external_function(
        set_resolution_scale_external_fn_id,
        move |mem_values: &[VariableValue], context| {
            let params = convert_to_values(mem_values)
                .expect("external function should be given values and no references");
            let scale = params[1].expect_int()?;

            context.display.set_scale(scale as u16);

            Ok(Value::Unit)
        },
    )?;

    Ok(gfx_value_mut)
}

/// # Errors
///
/// # Panics
///
#[allow(clippy::too_many_lines)]
pub fn register_asset_struct_value_with_members(
    state: &mut ProgramState,
    externals: &mut ExternalFunctions<ScriptRenderContext>,
    symbol_table: &mut SymbolTable,
    material_struct_type_ref: NamedStructTypeRef,
    fixed_grid_struct_type_ref: NamedStructTypeRef,
    font_and_material_struct_type_ref: &NamedStructTypeRef,
) -> Result<VariableValue, MangroveError> {
    let asset_type_id = state.allocate_number();
    let (assets_value, assets_type) =
        create_empty_struct_value_util(symbol_table, "Assets", asset_type_id)?;
    let assets_value_mut = VariableValue::Reference(Rc::new(RefCell::new(assets_value)));

    let assets_general_type = Type::NamedStruct(assets_type.clone());
    state.associated_impls.prepare(&assets_general_type);

    let mut_self_parameter = TypeForParameter {
        name: "self".to_string(),
        resolved_type: assets_general_type.clone(),
        is_mutable: true,
        node: None,
    };

    let string_type = Type::String;
    let asset_name_parameter = TypeForParameter {
        name: "asset_name".to_string(),
        resolved_type: string_type,
        is_mutable: false,
        node: None,
    };

    let material_png_function_id: ExternalFunctionId = state.allocate_external_function_id();
    let material_png_def = ExternalFunctionDefinition {
        name: None,
        assigned_name: "material_png".to_string(),
        signature: Signature {
            parameters: [mut_self_parameter.clone(), asset_name_parameter.clone()].to_vec(),
            return_type: Box::from(Type::NamedStruct(material_struct_type_ref)),
        },
        id: material_png_function_id,
    };

    state
        .associated_impls
        .add_external_member_function(&assets_general_type.clone(), material_png_def)?;

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

    let bm_font_function_id: ExternalFunctionId = state.allocate_external_function_id();
    let bm_font_def = ExternalFunctionDefinition {
        name: None,
        assigned_name: "bm_font".to_string(),
        signature: Signature {
            parameters: [mut_self_parameter.clone(), asset_name_parameter.clone()].to_vec(),
            return_type: Box::from(Type::NamedStruct(font_and_material_struct_type_ref.clone())),
        },
        id: bm_font_function_id,
    };

    state
        .associated_impls
        .add_external_member_function(&assets_general_type.clone(), bm_font_def)?;

    externals.register_external_function(
        bm_font_function_id,
        move |mem_values: &[VariableValue], context| {
            //let self_value = &params[0]; // Assets is, by design, an empty struct
            let params = convert_to_values(mem_values)
                .expect("should only be passed values to material png function");
            let asset_name = &params[1].expect_string()?;

            Ok(context.game_assets.as_mut().unwrap().bm_font(asset_name))
        },
    )?;

    let int_type = Type::Int;
    let size_int_tuple_type = vec![int_type.clone(), int_type];

    let size_param = TypeForParameter {
        name: String::default(),
        resolved_type: Type::Tuple(size_int_tuple_type),
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

    let frame_fixed_grid_material_png_def = ExternalFunctionDefinition {
        name: None,
        assigned_name: "frame_fixed_grid_material_png".to_string(),
        signature: Signature {
            parameters: [
                mut_self_parameter,
                asset_name_parameter,
                size_param.clone(),
                size_param.clone(),
            ]
            .to_vec(),
            return_type: Box::from(Type::NamedStruct(fixed_grid_struct_type_ref)),
        },
        id: frame_fixed_grid_material_png_function_id,
    };

    state.associated_impls.add_external_member_function(
        &assets_general_type.clone(),
        frame_fixed_grid_material_png_def,
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
    render_fn: InternalFunctionDefinitionRef,
    externals: ExternalFunctions<ScriptRenderContext>,
    display_settings: DisplaySettings,
    constants: Constants,
    gfx_struct_ref: ValueRef,
}

impl ScriptRender {
    /// # Errors
    ///
    pub fn new(
        render_value_ref: ValueRef,
        render_struct_type_ref: &NamedStructTypeRef,
        externals: ExternalFunctions<ScriptRenderContext>,
        constants: Constants,
        impls: AssociatedImpls,
        gfx_struct_ref: ValueRef,
    ) -> Result<Self, MangroveError> {
        let render_fn = get_impl_func(&impls, render_struct_type_ref, "render");

        Ok(Self {
            render_value_ref,
            render_fn,
            externals,
            display_settings: DisplaySettings::new(),
            constants,
            gfx_struct_ref,
        })
    }

    /// # Errors
    ///
    pub fn render(
        &mut self,
        wgpu_render: &mut Render,
        simulation_value_ref: &Value,
        _source_map_wrapper: &SourceMapWrapper,
    ) -> Result<(), ExecuteError> {
        let mut script_context = ScriptRenderContext {
            game_assets: None,
            render: Some(RenderWrapper::new(wgpu_render)),
            display: DisplaySettings::new(),
        };

        let self_mut_ref = VariableValue::Reference(self.render_value_ref.clone());

        util_execute_function(
            &self.externals,
            &self.constants,
            &self.render_fn,
            [
                self_mut_ref, //   self.render_value_ref.clone()
                VariableValue::Value(simulation_value_ref.clone()),
                VariableValue::Reference(self.gfx_struct_ref.clone()),
            ]
            .as_ref(),
            &mut script_context,
            None,
        )?;

        self.display_settings = script_context.display;

        Ok(())
    }
}

/// # Errors
///
pub fn create_render_module(
    resolved_program: &mut Program,
    externals: &mut ExternalFunctions<ScriptRenderContext>,
) -> Result<
    (
        SymbolTable,
        VariableValue,
        ValueRef,
        NamedStructTypeRef,
        ExternalTypeRef,
        NamedStructTypeRef,
        ExternalTypeRef,
        NamedStructTypeRef,
        ExternalTypeRef,
    ),
    MangroveError,
> {
    //    let mut mangrove_render_module = Module::new(&["mangrove".to_string(), "render".to_string()]);
    let mut symbol_table = SymbolTable::new();

    let material_handle_rust_type_ref = Rc::new(ExternalType {
        type_name: "MaterialHandle".to_string(),
        number: 91,
    });

    let handle_type_id = resolved_program.state.allocate_number();
    let material_handle_struct_ref = symbol_table.add_generated_struct(
        "MaterialHandle",
        &[(
            "hidden",
            Type::External(material_handle_rust_type_ref.clone()),
        )],
        handle_type_id,
    )?;

    let fixed_atlas_handle_rust_type_ref = Rc::new(ExternalType {
        type_name: "FixedAtlasHandle".to_string(),
        number: 92,
    });

    let fixed_atlas_handle_type_id = resolved_program.state.allocate_number();
    let fixed_atlas_handle_struct_ref = symbol_table.add_generated_struct(
        "FixedAtlasHandle",
        &[(
            "hidden",
            Type::External(fixed_atlas_handle_rust_type_ref.clone()),
        )],
        fixed_atlas_handle_type_id,
    )?;

    let font_and_material_rust_type_ref = symbol_table.add_external_type(ExternalType {
        type_name: "FontAndMaterialHandleType".to_string(),
        number: 0,
    })?;

    let font_and_material_handle = resolved_program.state.allocate_number();

    let font_and_material_struct_ref = symbol_table.add_generated_struct(
        "FontAndMaterialHandle",
        &[(
            "hidden",
            Type::External(font_and_material_rust_type_ref.clone()),
        )],
        font_and_material_handle,
    )?;

    let assets_struct_value = register_asset_struct_value_with_members(
        &mut resolved_program.state,
        externals,
        &mut symbol_table,
        material_handle_struct_ref.clone(),
        fixed_atlas_handle_struct_ref.clone(),
        &font_and_material_struct_ref,
    )?;

    let gfx_struct_value = register_gfx_struct_value_with_members(
        &mut resolved_program.state,
        externals,
        &mut symbol_table,
    )?;

    Ok((
        symbol_table,
        assets_struct_value,
        gfx_struct_value,
        material_handle_struct_ref,
        material_handle_rust_type_ref,
        fixed_atlas_handle_struct_ref,
        fixed_atlas_handle_rust_type_ref,
        font_and_material_struct_ref,
        font_and_material_rust_type_ref,
    ))
}

/// # Errors
///
/// # Panics
///
pub fn boot(
    resource_storage: &mut ResourceStorage,
    simulation_main_module: &ModuleRef,
) -> Result<ScriptRender, MangroveError> {
    let mut resolved_program = Program::new();
    let mut external_functions = ExternalFunctions::<ScriptRenderContext>::new();

    let (
        mangrove_render_symbol_table,
        assets_value,
        gfx_value,
        material_handle_struct_ref,
        material_handle_rust_type_ref,
        fixed_atlas_struct_type_ref,
        fixed_atlas_handle_rust_type_ref,
        font_and_material_struct_type_ref,
        font_and_material_rust_type_ref,
    ) = create_render_module(&mut resolved_program, &mut external_functions)?;
    let mangrove_render_module_path = &["mangrove".to_string(), "render".into()];

    let render_module_ref = Module::new(
        mangrove_render_module_path,
        mangrove_render_symbol_table,
        None,
    );
    resolved_program
        .modules
        .add(ModuleRef::from(render_module_ref));

    resolved_program.modules.add(simulation_main_module.clone());
    resolved_program
        .modules
        .link_module(&["simulation".to_string()], simulation_main_module.clone());

    let crate_render_module_path = &["crate".to_string(), "render".into()];
    {
        let source_map = resource_storage.fetch_mut::<SourceMapResource>();
        compile(
            crate_render_module_path,
            &mut resolved_program,
            &mut external_functions,
            &mut source_map.wrapper.source_map,
        )?;
    };

    let main_module = resolved_program
        .modules
        .get(crate_render_module_path)
        .expect("could not find main module");

    let main_fn = main_module
        .namespace
        .symbol_table
        .get_internal_function("main")
        .expect("No main function");

    let mut game_assets = GameAssets::new(resource_storage, Millis::new(0));

    let mut script_context = ScriptRenderContext {
        game_assets: Some(GameAssetsWrapper::new(
            &mut game_assets,
            material_handle_struct_ref,
            material_handle_rust_type_ref,
            fixed_atlas_struct_type_ref,
            fixed_atlas_handle_rust_type_ref,
            font_and_material_struct_type_ref,
            font_and_material_rust_type_ref,
        )),
        render: None,
        display: DisplaySettings::new(),
    };

    let mut constants = Constants::new();
    eval_constants(
        &external_functions,
        &mut constants,
        &resolved_program.state,
        &mut script_context,
    )?;

    //let source_map = resource_storage.fetch::<SourceMapResource>();
    let render_struct_value = util_execute_function(
        &external_functions,
        &constants,
        main_fn,
        &[assets_value],
        &mut script_context,
        None, //Some(&source_map.wrapper),
    )?;

    let Value::NamedStruct(render_struct_type_ref, _) = render_struct_value.clone() else {
        return Err(MangroveError::Other(
            "needs to be simulation struct".to_string(),
        ));
    };

    // let render_fn = get_impl_func(&render_struct_type_ref, "render");

    // Convert it to a mutable (reference), so it can be mutated in update ticks
    let render_struct_value_mutable_ref = Rc::new(RefCell::new(render_struct_value));

    ScriptRender::new(
        render_struct_value_mutable_ref,
        &render_struct_type_ref,
        external_functions,
        constants,
        resolved_program.state.associated_impls,
        gfx_value,
    )
}

/// # Panics
///
pub fn update_screen_resolution_tick(
    mut script: LoReM<ScriptRender>,
    mut wgpu_render: ReM<Render>,
    mut window_settings: ReM<limnus_screen::Window>,
) {
    if script.display_settings.scale != 0 {
        let new_size = wgpu_render.virtual_surface_size() * script.display_settings.scale;
        debug!(?new_size, "resizing");
        window_settings.requested_surface_size = new_size;
    }
}

/// # Panics
///
pub fn render_tick(
    mut script: LoReM<ScriptRender>,
    simulation: LoRe<ScriptSimulation>,
    mut wgpu_render: ReM<Render>,

    error: Re<ErrorResource>,
    source_map: Re<SourceMapResource>,
) {
    if error.has_errors {
        return;
    }
    script
        .render(
            &mut wgpu_render,
            &simulation.immutable_simulation_value(),
            &source_map.wrapper,
        )
        .expect("script.render() crashed");
}

pub fn detect_reload_tick(
    script_messages: Msg<ScriptMessage>,
    mut script_render: LoReM<ScriptRender>,
    script_simulation: LoRe<ScriptSimulation>, // it is important that reload is done in correct order, so the new simulation should exist
    source_map: Re<SourceMapResource>,
    mut all_resources: ReAll,
    mut err: ReM<ErrorResource>,
) {
    if err.has_errors {
        return;
    }
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => {
                match boot(&mut all_resources, &script_simulation.main_module()) {
                    Ok(new_render) => *script_render = new_render,
                    Err(mangrove_error) => {
                        err.has_errors = true;
                        show_mangrove_error(&mangrove_error, &source_map.wrapper.source_map);
                        eprintln!("script render failed: {mangrove_error}");
                        error!(error=?mangrove_error, "script render failed");
                    }
                }
            }
        }
    }
}

pub struct ScriptRenderPlugin;

impl Plugin for ScriptRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(Update, detect_reload_tick);
        app.add_system(Update, update_screen_resolution_tick);
        app.add_system(RenderUpdate, render_tick);
        // HACK: Just add a completely zeroed out ScriptRender and wait for reload message.
        // TODO: Should not try to call updates with params that are not available yet.
        app.insert_local_resource(ScriptRender {
            render_value_ref: Rc::new(RefCell::new(Value::default())),
            render_fn: Rc::new(InternalFunctionDefinition {
                body: Expression {
                    ty: Type::Int,
                    node: Default::default(),
                    kind: ExpressionKind::Break,
                },
                name: LocalIdentifier(Node::default()),
                assigned_name: "".to_string(),
                signature: Signature {
                    parameters: vec![],
                    return_type: Box::from(Type::Unit),
                },
            }),
            externals: ExternalFunctions::new(),
            display_settings: DisplaySettings::new(),
            constants: Constants { values: vec![] },
            gfx_struct_ref: Rc::new(RefCell::new(Value::default())),
        });
    }
}
