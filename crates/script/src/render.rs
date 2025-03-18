/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::err::show_mangrove_error;
use crate::script::{
    MangroveError, color_like, create_empty_struct_value, register_print, sprite_params,
    uvec2_like, vec3_like,
};
use crate::script_main::ScriptMain;
use crate::simulation::ScriptSimulation;
use crate::util::get_impl_func;
use crate::{ErrorResource, ScriptMessage, SourceMapResource};
use monotonic_time_rs::Millis;
use std::cell::RefCell;
use std::env::current_dir;
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;
use swamp::prelude::{
    App, Assets, Color, FixedAtlas, FontAndMaterial, FrameLookup, GameAssets, Gfx, LoRe, LoReM,
    LocalResource, MaterialRef, Msg, Plugin, Re, ReAll, ReM, Render, RenderUpdate, ResourceStorage,
    SpriteParams, UVec2, Update, Vec3, ViewportStrategy,
};
use swamp_script::prelude::*;
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
        corner_size_in_texture: UVec2,
        window_size_in_texture: UVec2,
        material: &MaterialRef,
        atlas_offset: UVec2,
        color: Color,
    ) {
        // Safety: We assume the Render pointer is still valid, since the RenderWrapper is short-lived (only alive during a render call)
        let render: &mut Render;
        unsafe {
            render = &mut *self.render;
        }

        render.draw_nine_slice(pos, size, corner_size_in_texture, window_size_in_texture, material, atlas_offset, color);
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

    struct_types: HandleNamedStructTypes,
    external_types: ExternalTypes,
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

#[derive(PartialEq, Eq)]
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

        struct_types: HandleNamedStructTypes,
        external_types: ExternalTypes,
    ) -> Self {
        let ptr = game_assets as *mut GameAssets;
        Self {
            game_assets: ptr as *mut GameAssets<'static>, // Coerce to 'static. is there a better way?
            struct_types,
            external_types,
        }
    }

    fn material_handle(&self, material_ref: MaterialRef) -> Value {
        let material_ref_value = Rc::new(RefCell::new(Value::RustValue(
            self.external_types.material.clone(),
            Rc::new(RefCell::new(Box::new(MaterialWrapper(material_ref)))),
        )));

        Value::NamedStruct(
            self.struct_types.material.clone(),
            [material_ref_value].to_vec(),
        )
    }

    fn fixed_atlas_handle(&self, fixed_atlas: FixedAtlas) -> Value {
        let wrapper = FixedAtlasWrapper { fixed_atlas };
        let fixed_atlas_ref = Rc::new(RefCell::new(Value::RustValue(
            self.external_types.fixed_atlas.clone(),
            Rc::new(RefCell::new(Box::new(wrapper))),
        )));

        Value::NamedStruct(
            self.struct_types.fixed_atlas.clone(),
            [fixed_atlas_ref].to_vec(),
        )
    }

    fn font_and_material_handle(&self, font_and_material: FontAndMaterial) -> Value {
        let wrapper = FontAndMaterialWrapper { font_and_material };
        let font_and_material_ref = Rc::new(RefCell::new(Value::RustValue(
            self.external_types.font_and_material.clone(),
            Rc::new(RefCell::new(Box::new(wrapper))),
        )));

        Value::NamedStruct(
            self.struct_types.font_and_material.clone(),
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
pub fn register_assets_members(
    symbol_table: &SymbolTable,
    associated_impls: &AssociatedImpls,
    externals: &mut ExternalFunctions<ScriptRenderContext>,
) -> Result<(), String> {
    let assets_struct = symbol_table.get_type("Assets").unwrap();

    // material_png
    let material_png_function_id =
        associated_impls.api_fetch_external_function_id(assets_struct, "material_png");
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

    // material_png
    let bm_font_function_id =
        associated_impls.api_fetch_external_function_id(assets_struct, "bm_font");
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

    // frame_fixed_grid_material_png
    let frame_fixed_grid_material_png_function_id = associated_impls
        .api_fetch_external_function_id(assets_struct, "frame_fixed_grid_material_png");
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

    Ok(())
}

/// # Errors
///
/// # Panics
///
#[allow(clippy::too_many_lines)]
pub fn register_gfx_members(
    symbol_table: &SymbolTable,
    associated_impls: &AssociatedImpls,
    externals: &mut ExternalFunctions<ScriptRenderContext>,
) -> Result<(), String> {
    let gfx_struct = symbol_table.get_type("Gfx").unwrap();

    // sprite
    let sprite_fn_id = associated_impls.api_fetch_external_function_id(gfx_struct, "sprite");
    externals.register_external_function(
        sprite_fn_id,
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

    // sprite_ex
    let sprite_ex_external_fn_id =
        associated_impls.api_fetch_external_function_id(gfx_struct, "sprite_ex");
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

    // quad
    let quad_external_fn_id = associated_impls.api_fetch_external_function_id(gfx_struct, "quad");
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

    // nine_slice
    let nine_slice_external_fn_id =
        associated_impls.api_fetch_external_function_id(gfx_struct, "nine_slice");
    externals.register_external_function(
        nine_slice_external_fn_id,
        move |mem_val: &[VariableValue], context| {
            let params = convert_to_values(mem_val).unwrap();
            //let _self_value = &params[0]; // the Gfx struct is empty by design.
            let position = vec3_like(&params[1])?;
            let window_size_on_screen = uvec2_like(&params[2])?;
            let corner_size_in_texture = uvec2_like(&params[3])?;
            let window_size_in_texture = uvec2_like(&params[4])?;
            let material_ref = params[5].downcast_hidden_rust::<MaterialWrapper>().unwrap();
            let atlas_offset = uvec2_like(&params[6])?;
            let color = color_like(&params[7])?;

            context.render.as_mut().unwrap().push_nine_slice(
                position,
                window_size_on_screen,
                corner_size_in_texture,
                window_size_in_texture,
                &material_ref.borrow().0,
                atlas_offset,
                color,
            );

            Ok(Value::Unit)
        },
    )?;

    // text
    let text_external_fn_id = associated_impls.api_fetch_external_function_id(gfx_struct, "text");
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

    // sprite atlas frame
    let sprite_atlas_frame_external_fn_id =
        associated_impls.api_fetch_external_function_id(gfx_struct, "sprite_atlas_frame");
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

    let sprite_atlas_frame_ex_external_fn_id =
        associated_impls.api_fetch_external_function_id(gfx_struct, "sprite_atlas_frame_ex");
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

    // set_virtual_resolution
    let set_virtual_resolution_external_fn_id =
        associated_impls.api_fetch_external_function_id(gfx_struct, "set_virtual_resolution");
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

    // set_resolution_scale
    let set_resolution_scale_external_fn_id =
        associated_impls.api_fetch_external_function_id(gfx_struct, "set_resolution_scale");
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

    Ok(())
}

#[derive(LocalResource, Debug)]
pub struct ScriptRender {
    render_value_ref: ValueRef,
    render_fn: InternalFunctionDefinitionRef,
    externals: ExternalFunctions<ScriptRenderContext>,
    display_settings: DisplaySettings,
    gfx_struct_ref: ValueRef,
}

impl ScriptRender {
    /// # Errors
    ///
    pub fn new(
        render_value_ref: ValueRef,
        render_struct_type_ref: &NamedStructType,
        externals: ExternalFunctions<ScriptRenderContext>,
        impls: &AssociatedImpls,
        gfx_struct_ref: ValueRef,
    ) -> Result<Self, MangroveError> {
        let render_fn = get_impl_func(impls, render_struct_type_ref, "render");

        Ok(Self {
            render_value_ref,
            render_fn,
            externals,
            display_settings: DisplaySettings::new(),
            gfx_struct_ref,
        })
    }

    /// # Errors
    ///
    pub fn render(
        &mut self,
        script_main: &ScriptMain,
        wgpu_render: &mut Render,
        simulation_value_ref: &Value,
        _source_map_wrapper: &SourceMapWrapper,
    ) -> Result<(), RuntimeError> {
        let mut script_context = ScriptRenderContext {
            game_assets: None,
            render: Some(RenderWrapper::new(wgpu_render)),
            display: DisplaySettings::new(),
        };

        let self_mut_ref = VariableValue::Reference(self.render_value_ref.clone());

        util_execute_function(
            &self.externals,
            &script_main.constants,
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

#[derive(Clone, Debug)]
pub struct HandleNamedStructTypes {
    pub material: NamedStructType,
    pub fixed_atlas: NamedStructType,
    pub font_and_material: NamedStructType,
    pub assets: NamedStructType,
    pub gfx: NamedStructType,
}

/// # Errors
///
/// # Panics
///
pub fn boot(
    script_main: &ScriptMain,
    resource_storage: &mut ResourceStorage,
) -> Result<ScriptRender, MangroveError> {
    debug!("boot script render");
    //    let mut resolved_program = Program::new();
    let mut external_functions = ExternalFunctions::<ScriptRenderContext>::new();
    let mangrove_render_module_path = &["mangrove".to_string(), "render".into()];
    let mangrove_render_module = script_main
        .resolved_program
        .modules
        .get(mangrove_render_module_path)
        .unwrap();
    let impls = &script_main.resolved_program.state.associated_impls;

    register_print(
        &script_main.resolved_program.modules,
        &mut external_functions,
    );

    register_assets_members(
        &mangrove_render_module.symbol_table,
        impls,
        &mut external_functions,
    )?;

    register_gfx_members(
        &mangrove_render_module.symbol_table,
        impls,
        &mut external_functions,
    )?;

    let handle_named_types = get_handle_types(&mangrove_render_module.symbol_table);
    let handle_external_types = create_external_types();

    let mut game_assets = GameAssets::new(resource_storage, Millis::new(0));

    let mut script_context = ScriptRenderContext {
        game_assets: Some(GameAssetsWrapper::new(
            &mut game_assets,
            handle_named_types.clone(),
            handle_external_types,
        )),
        render: None,
        display: DisplaySettings::new(),
    };

    let mut constants = Constants::new();
    eval_constants(
        &external_functions,
        &mut constants,
        &script_main.resolved_program.state,
        &mut script_context,
    )?;

    //let source_map = resource_storage.fetch::<SourceMapResource>();
    // Assets doesn't really contain anything, the actual values are fetched from the runtime context
    let assets_value = create_empty_struct_value(handle_named_types.assets.clone());
    let assets_value_mut = VariableValue::Reference(Rc::new(RefCell::new(assets_value)));

    let render_struct_value = util_execute_function(
        &external_functions,
        &constants,
        &script_main.render_new_fn,
        &[assets_value_mut],
        &mut script_context,
        None, //Some(&source_map.wrapper),
    )?;

    let Value::NamedStruct(render_struct_type_ref, _) = render_struct_value.clone() else {
        return Err(MangroveError::Other(
            "needs to be simulation struct".to_string(),
        ));
    };

    // Gfx doesn't really contain anything, the actual values are fetched from the runtime context
    let gfx_value = create_empty_struct_value(handle_named_types.gfx.clone());
    let gfx_value_mut = Rc::new(RefCell::new(gfx_value));
    // let render_fn = get_impl_func(&render_struct_type_ref, "render");

    // Convert it to a mutable (reference), so it can be mutated in update ticks
    let render_struct_value_mutable_ref = Rc::new(RefCell::new(render_struct_value));

    ScriptRender::new(
        render_struct_value_mutable_ref,
        &render_struct_type_ref,
        external_functions,
        impls,
        gfx_value_mut,
    )
}

#[derive(Debug)]
pub struct ExternalTypes {
    pub material: ExternalType,
    pub font_and_material: ExternalType,
    pub fixed_atlas: ExternalType,
}

fn create_external_types() -> ExternalTypes {
    let material = ExternalType {
        type_name: "MaterialHandle".to_string(),
        number: 91,
    };

    let font_and_material = ExternalType {
        type_name: "FontAndMaterial".to_string(),
        number: 92,
    };

    let fixed_atlas = ExternalType {
        type_name: "FixedAtlas".to_string(),
        number: 93,
    };

    ExternalTypes {
        material,
        font_and_material,
        fixed_atlas,
    }
}

fn get_handle_types(mangrove_render_symbol_table: &SymbolTable) -> HandleNamedStructTypes {
    let font_and_material = mangrove_render_symbol_table
        .get_struct("FontAndMaterialHandle")
        .unwrap()
        .clone();
    let fixed_atlas = mangrove_render_symbol_table
        .get_struct("FixedAtlasHandle")
        .unwrap()
        .clone();
    let material = mangrove_render_symbol_table
        .get_struct("MaterialHandle")
        .unwrap()
        .clone();

    let assets = mangrove_render_symbol_table
        .get_struct("Assets")
        .unwrap()
        .clone();

    let gfx = mangrove_render_symbol_table
        .get_struct("Gfx")
        .unwrap()
        .clone();

    HandleNamedStructTypes {
        material,
        fixed_atlas,
        font_and_material,
        assets,
        gfx,
    }
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
        window_settings.requested_surface_size = new_size;
    }
}

/// # Panics
///
pub fn render_tick(
    script_main: LoRe<ScriptMain>,
    mut script: LoReM<ScriptRender>,
    simulation: LoRe<ScriptSimulation>,
    mut wgpu_render: ReM<Render>,

    mut error: ReM<ErrorResource>,
    source_map: Re<SourceMapResource>,
) {
    if error.has_errors {
        return;
    }
    script
        .render(
            &script_main,
            &mut wgpu_render,
            &simulation.immutable_simulation_value(),
            &source_map.wrapper(),
        )
        .inspect_err(|runtime_err| {
            error.has_errors = true;
            let current_path = &*current_dir().unwrap();
            show_runtime_error(runtime_err, &source_map.source_map, &current_path)
        });
}

pub fn detect_reload_tick(
    script_messages: Msg<ScriptMessage>,
    mut script_render: LoReM<ScriptRender>,
    script_main: LoRe<ScriptMain>,
    source_map: Re<SourceMapResource>,
    mut all_resources: ReAll,
    mut err: ReM<ErrorResource>,
) {
    if err.has_errors {
        return;
    }
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => match boot(&script_main, &mut all_resources) {
                Ok(new_render) => *script_render = new_render,
                Err(mangrove_error) => {
                    err.has_errors = true;
                    show_mangrove_error(&mangrove_error, &source_map.source_map);
                    eprintln!("script render failed: {mangrove_error}");
                    error!(error=?mangrove_error, "script render failed");
                }
            },
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
                    node: Node::default(),
                    kind: ExpressionKind::Block(vec![]),
                },
                name: LocalIdentifier(Node::default()),
                assigned_name: String::new(),
                signature: Signature {
                    parameters: vec![],
                    return_type: Box::from(Type::Unit),
                },
                variable_scopes: FunctionScopeState::new(Type::Unit),
                function_scope_state: Vec::default(),
                program_unique_id: 0,
            }),
            externals: ExternalFunctions::new(),
            display_settings: DisplaySettings::new(),
            gfx_struct_ref: Rc::new(RefCell::new(Value::default())),
        });
    }
}
