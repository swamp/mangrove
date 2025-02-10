/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::err::show_mangrove_error;
use crate::script::{
    color_like, compile, create_empty_struct_value, sprite_params, uvec2_like, vec3_like,
    MangroveError,
};
use crate::simulation::ScriptSimulation;
use crate::util::get_impl_func;
use crate::{ErrorResource, ScriptMessage, SourceMapResource};
use monotonic_time_rs::Millis;
use std::cell::RefCell;
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;
use swamp::prelude::{
    App, Assets, Color, FixedAtlas, FontAndMaterial, FrameLookup, GameAssets, Gfx, LoRe, LoReM,
    LocalResource, MaterialRef, Msg, Plugin, Re, ReAll, ReM, Render, RenderUpdate, ResourceStorage,
    SpriteParams, UVec2, Update, Vec3,
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

        render.draw_nine_slice(
            pos,
            size,
            corner_size,
            texture_window_size,
            material,
            atlas_offset,
            color,
        );
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

    font_and_material_struct_type_ref: ResolvedStructTypeRef,
    font_and_material_rust_type_ref: ResolvedRustTypeRef,
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
        material_struct_type: ResolvedStructTypeRef,
        material_rust_type_ref: ResolvedRustTypeRef,
        fixed_atlas_struct_type_ref: ResolvedStructTypeRef,
        fixed_atlas_rust_type_ref: ResolvedRustTypeRef,

        font_and_material_struct_type_ref: ResolvedStructTypeRef,
        font_and_material_rust_type_ref: ResolvedRustTypeRef,
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
            [fixed_atlas_ref].to_vec(),
        )
    }

    fn font_and_material_handle(&self, font_and_material: FontAndMaterial) -> Value {
        let wrapper = FontAndMaterialWrapper { font_and_material };
        let font_and_material_ref = Rc::new(RefCell::new(Value::RustValue(
            self.font_and_material_rust_type_ref.clone(),
            Rc::new(RefCell::new(Box::new(wrapper))),
        )));

        Value::Struct(
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
/// # Panics
///
#[allow(clippy::too_many_lines)]
pub fn register_gfx_external_functions(
    externals: &mut ExternalFunctions<ScriptRenderContext>,
    namespace: &mut ResolvedModuleNamespace,
) -> Result<ValueRef, MangroveError> {
    let gfx_struct = namespace.fetch_struct("Gfx");

    let gfx_value = create_empty_struct_value(gfx_struct.clone());
    let gfx_value_mut = Rc::new(RefCell::new(gfx_value));
    let borrow = gfx_struct.borrow_mut();

    // sprite() ---------------------------
    let sprite_external_fn_id: ExternalFunctionId = borrow.fetch_external_function_id("sprite");
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
    let sprite_ex_external_fn_id: ExternalFunctionId =
        borrow.fetch_external_function_id("sprite_ex");

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

    // quad() ---------------------------
    let quad_external_fn_id: ExternalFunctionId = borrow.fetch_external_function_id("quad");
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

    // nine_slice() ---------------------------
    // https://en.wikipedia.org/wiki/9-slice_scaling
    let nine_slice_external_fn_id: ExternalFunctionId = borrow.fetch_external_function_id("quad");

    externals.register_external_function(
        nine_slice_external_fn_id,
        move |mem_val: &[VariableValue], context| {
            let params = convert_to_values(mem_val).unwrap();
            //let _self_value = &params[0]; // the Gfx struct is empty by design.
            let position = vec3_like(&params[1])?;
            let complete_size = uvec2_like(&params[2])?;
            let corner_size = uvec2_like(&params[3])?;
            let texture_window_size = uvec2_like(&params[4])?;
            let material_ref = params[5].downcast_hidden_rust::<MaterialWrapper>().unwrap();
            let atlas_offset = uvec2_like(&params[6])?;
            let color = color_like(&params[7])?;

            context.render.as_mut().unwrap().push_nine_slice(
                position,
                complete_size,
                corner_size,
                texture_window_size,
                &material_ref.borrow().0,
                atlas_offset,
                color,
            );

            Ok(Value::Unit)
        },
    )?;

    // text() ---------------------------
    let text_external_fn_id: ExternalFunctionId = borrow.fetch_external_function_id("text");
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

    // sprite_atlas_frame ---------------------------
    let sprite_atlas_frame_external_fn_id: ExternalFunctionId =
        borrow.fetch_external_function_id("sprite_atlas_frame");

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
        borrow.fetch_external_function_id("sprite_atlas_frame_ex");

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

    Ok(gfx_value_mut)
}

/// # Errors
///
/// # Panics
///
#[allow(clippy::too_many_lines)]
pub fn register_assets_external_functions(
    externals: &mut ExternalFunctions<ScriptRenderContext>,
    namespace: &mut ResolvedModuleNamespace,
) -> Result<VariableValue, MangroveError> {
    let assets_struct = namespace.fetch_struct("Assets");
    let assets_borrow = assets_struct.borrow();

    let material_png_function_id: ExternalFunctionId =
        assets_borrow.fetch_external_function_id("material_png");

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

    // bm_font
    let bm_font_function_id: ExternalFunctionId =
        assets_borrow.fetch_external_function_id("bm_font");
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
    let frame_fixed_grid_material_png_function_id: ExternalFunctionId =
        assets_borrow.fetch_external_function_id("frame_fixed_grid_material_png");

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

#[derive(LocalResource, Debug)]
pub struct ScriptRender {
    render_value_ref: ValueRef,
    render_fn: ResolvedInternalFunctionDefinitionRef,
    externals: ExternalFunctions<ScriptRenderContext>,
    constants: Constants,
    gfx_struct_ref: ValueRef,
}

impl ScriptRender {
    /// # Errors
    ///
    pub fn new(
        render_value_ref: ValueRef,
        render_struct_type_ref: &ResolvedStructTypeRef,
        externals: ExternalFunctions<ScriptRenderContext>,
        constants: Constants,
        gfx_struct_ref: ValueRef,
    ) -> Result<Self, MangroveError> {
        let render_fn = get_impl_func(render_struct_type_ref, "render");

        Ok(Self {
            render_value_ref,
            render_fn,
            externals,
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

        Ok(())
    }
}

/// # Errors
///
pub fn fetch_mangrove_render_module_handles(
    borrow: &mut ResolvedModuleNamespace,
) -> Result<
    (
        ResolvedStructTypeRef,
        ResolvedRustTypeRef,
        ResolvedStructTypeRef,
        ResolvedRustTypeRef,
        ResolvedStructTypeRef,
        ResolvedRustTypeRef,
    ),
    MangroveError,
> {
    let material_handle_rust_type_ref = Rc::new(ResolvedRustType {
        type_name: "MaterialHandleGen".to_string(),
        number: 91,
    });
    let material_handle_struct_ref = &borrow.add_generated_struct(
        "MaterialHandleGen",
        &[(
            "hidden",
            ResolvedType::RustType(material_handle_rust_type_ref.clone()),
        )],
    )?;

    let fixed_atlas_handle_rust_type_ref = Rc::new(ResolvedRustType {
        type_name: "FixedAtlasHandleGen".to_string(),
        number: 92,
    });

    let fixed_atlas_handle_struct_ref = borrow.fetch_struct("FixedAtlasHandle");
    let font_and_material_struct_ref = borrow.fetch_struct("FontAndMaterialHandle");
    let material_handle_struct_ref = borrow.fetch_struct("MaterialHandle");

    Ok((
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
    simulation_main_module: &ResolvedModuleRef,
) -> Result<ScriptRender, MangroveError> {
    let mut resolved_program = ResolvedProgram::new();
    let mut external_functions = ExternalFunctions::<ScriptRenderContext>::new();

    //let render_module_ref = Rc::new(RefCell::new(render_module));
    //resolved_program.modules.add(render_module_ref);
    resolved_program.modules.add(simulation_main_module.clone());

    {
        let source_map = resource_storage.fetch_mut::<SourceMapResource>();
        let base_path = { source_map.base_path().to_path_buf() };
        compile(
            base_path.as_path(),
            &["render".to_string()],
            &mut resolved_program,
            &mut external_functions,
            &mut source_map.wrapper.source_map,
        )?;
    };

    let root_module_path = &["render".to_string()].to_vec();
    let user_render_module = resolved_program
        .modules
        .get(root_module_path)
        .expect("could not find main module");

    let render_module_borrow = user_render_module.borrow();
    let namespace_borrow = render_module_borrow.namespace.borrow_mut();
    let main_fn = namespace_borrow
        .get_internal_function("main")
        .expect("No main function");

    let mut game_assets = GameAssets::new(resource_storage, Millis::new(0));

    let mangrove_render_path = &["mangrove".to_string(), "render".to_string()].to_vec();
    let mangrove_render = resolved_program.modules.get(mangrove_render_path).unwrap();
    let mangrove_render_ns = &*mangrove_render.borrow().namespace.borrow();

    let (
        material_handle_struct_ref,
        material_handle_rust_type_ref,
        fixed_atlas_struct_type_ref,
        fixed_atlas_handle_rust_type_ref,
        font_and_material_struct_type_ref,
        font_and_material_rust_type_ref,
    ) = fetch_mangrove_render_module_handles(mangrove_render_ns)?;

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
    };

    let mut constants = Constants::new();
    eval_constants(
        &external_functions,
        &mut constants,
        &resolved_program.modules,
        &mut script_context,
    )?;

    //let source_map = resource_storage.fetch::<SourceMapResource>();

    let assets_value = create_empty_struct_value(assets_struct.clone());
    let assets_value_mut = VariableValue::Reference(Rc::new(RefCell::new(assets_value)));

    let render_struct_value = util_execute_function(
        &external_functions,
        &constants,
        main_fn,
        &[assets_value_mut],
        &mut script_context,
        None, //Some(&source_map.wrapper),
    )?;

    let Value::Struct(render_struct_type_ref, _) = render_struct_value.clone() else {
        return Err(MangroveError::Other(
            "needs to be simulation struct".to_string(),
        ));
    };

    // let render_fn = get_impl_func(&render_struct_type_ref, "render");

    // Convert it to a mutable (reference), so it can be mutated in update ticks
    let render_struct_value_mutable_ref = Rc::new(RefCell::new(render_struct_value));

    let gfx_value = create_empty_struct_value(gfx_struct.clone());
    let gfx_value_ref = Rc::new(RefCell::new(gfx_value));
    let gfx_value_mut = VariableValue::Reference(gfx_value_ref.clone());

    ScriptRender::new(
        render_struct_value_mutable_ref,
        &render_struct_type_ref,
        external_functions,
        constants,
        gfx_value_ref,
    )
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
        app.add_system(RenderUpdate, render_tick);
        // HACK: Just add a completely zeroed out ScriptRender and wait for reload message.
        // TODO: Should not try to call updates with params that are not available yet.
        app.insert_local_resource(ScriptRender {
            render_value_ref: Rc::new(RefCell::new(Value::default())),
            render_fn: Rc::new(ResolvedInternalFunctionDefinition {
                body: ResolvedExpression {
                    ty: ResolvedType::Int,
                    node: Default::default(),
                    kind: ResolvedExpressionKind::Break,
                },
                name: ResolvedLocalIdentifier(ResolvedNode::default()),
                signature: FunctionTypeSignature {
                    parameters: vec![],
                    return_type: Box::from(ResolvedType::Unit),
                },
            }),
            externals: ExternalFunctions::new(),
            constants: Constants { values: vec![] },
            gfx_struct_ref: Rc::new(RefCell::new(Value::default())),
        });
    }
}
