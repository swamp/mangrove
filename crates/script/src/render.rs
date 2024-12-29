use crate::logic::ScriptLogic;
use crate::script::{
    compile, create_empty_struct_value_util, uvec2_like, vec3_like, MangroveError,
};
use crate::util::get_impl_func;
use crate::{ScriptMessage, SourceMapResource};
use monotonic_time_rs::Millis;
use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use swamp::prelude::{
    App, Assets, FixedAtlas, FrameLookup, GameAssets, LoRe, LoReM, LocalResource, MaterialRef, Msg,
    Plugin, ReAll, ReM, Render, ResourceStorage, UVec2, UpdatePhase, Vec3,
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

    pub fn push_sprite(&self, pos: Vec3, material_ref: &MaterialRef, size: UVec2) {
        // Safety: We assume the Render pointer is still valid, since the RenderWrapper is short-lived (only alive during a render call)
        let render: &mut Render;
        unsafe {
            render = &mut *self.render;
        }

        render.draw_sprite(pos, size, material_ref);
    }

    pub fn sprite_atlas_frame(&mut self, position: Vec3, frame: u16, atlas: &impl FrameLookup) {
        // Safety: We assume the Render pointer is still valid, since the RenderWrapper is short-lived (only alive during a render call)
        let render: &mut Render;
        unsafe {
            render = &mut *self.render;
        }

        render.sprite_atlas_frame(position, frame, atlas);
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

pub fn register_gfx_struct_value_with_members(
    types: &ResolvedProgramTypes,
    state: &mut ResolvedProgramState,
    externals: &mut ExternalFunctions<ScriptRenderContext>,
    namespace: &mut ResolvedModuleNamespace,
) -> Result<ValueRef, MangroveError> {
    let gfx_type_number = state.allocate_number();
    let (gfx_value, gfx_struct_type) =
        create_empty_struct_value_util(namespace, "Gfx", gfx_type_number)?;
    let gfx_value_mut = Rc::new(RefCell::new(gfx_value.clone()));
    let assets_general_type = ResolvedType::Struct(gfx_struct_type.clone());

    let mut_self_parameter = ResolvedParameter {
        name: ResolvedNode::default(),
        resolved_type: assets_general_type.clone(),
        is_mutable: Some(ResolvedNode::default()),
    };

    let string_type = types.string_type();
    let material_handle = ResolvedParameter {
        name: Default::default(), // material handle
        resolved_type: string_type,
        is_mutable: None,
    };

    let int_type = types.int_type();
    let tuple_type = ResolvedTupleType(vec![int_type.clone(), int_type.clone(), int_type.clone()]);
    let tuple_type_ref = Rc::new(tuple_type);

    //position
    let position_param = ResolvedParameter {
        name: Default::default(),
        resolved_type: ResolvedType::Tuple(tuple_type_ref),
        is_mutable: None,
    };

    let size_int_tuple_type = ResolvedTupleType(vec![int_type.clone(), int_type.clone()]);
    let size_int_tuple_type_ref = Rc::new(size_int_tuple_type);

    // size
    let size_param = ResolvedParameter {
        name: Default::default(),
        resolved_type: ResolvedType::Tuple(size_int_tuple_type_ref),
        is_mutable: None,
    };

    let unique_id: ExternalFunctionId = state.allocate_external_function_id();

    let sprite_fn = ResolvedExternalFunctionDefinition {
        name: Default::default(),
        signature: ResolvedFunctionSignature {
            first_parameter_is_self: true,
            parameters: (&[
                mut_self_parameter.clone(),
                position_param.clone(),
                material_handle.clone(),
                size_param,
            ])
                .to_vec(),
            return_type: types.int_type(),
        },
        id: unique_id,
    };

    let _material_png_fn = gfx_struct_type.borrow_mut().add_external_member_function(
        "sprite",
        ResolvedExternalFunctionDefinitionRef::from(sprite_fn),
    )?;

    externals.register_external_function(
        "sprite",
        unique_id,
        move |mem_val: &[VariableValue], context| {
            let params = convert_to_values(mem_val).unwrap();
            //let _self_value = &params[0]; // the Gfx struct is empty by design.
            let position = vec3_like(&params[1])?;

            let material_ref = params[2].downcast_hidden_rust::<MaterialRef>().unwrap();

            let size = uvec2_like(&params[3])?;

            context
                .render
                .as_mut()
                .unwrap()
                .push_sprite(position, &material_ref.borrow(), size);

            Ok(Value::Unit)
        },
    )?;

    let unique_id: ExternalFunctionId = state.allocate_external_function_id();

    // frame
    let frame = ResolvedParameter {
        name: Default::default(),
        resolved_type: types.int_type(),
        is_mutable: None,
    };

    let material_png_fn_def = ResolvedExternalFunctionDefinition {
        name: Default::default(),
        signature: ResolvedFunctionSignature {
            first_parameter_is_self: true,
            parameters: (&[mut_self_parameter, position_param, material_handle, frame]).to_vec(),
            return_type: types.int_type(),
        },
        id: unique_id,
    };

    let _material_png_fn = gfx_struct_type
        .borrow_mut()
        .add_external_member_function("sprite_atlas_frame", material_png_fn_def.into())?;

    externals.register_external_function(
        "sprite_atlas_frame",
        unique_id,
        move |mem_values: &[VariableValue], context| {
            //let _self_value = &params[0]; // the Gfx struct is empty by design.
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
    let assets_type_number = state.allocate_number();
    let (assets_value, assets_type) =
        create_empty_struct_value_util(namespace, "Assets", assets_type_number)?;
    let assets_value_mut =
        VariableValue::Reference(ValueReference(Rc::new(RefCell::new(assets_value.clone()))));

    let assets_general_type = ResolvedType::Struct(assets_type.clone());

    let mut_self_parameter = ResolvedParameter {
        name: Default::default(),
        resolved_type: assets_general_type.clone(),
        is_mutable: Some(Default::default()),
    };

    let string_type = types.string_type();
    let asset_name_parameter = ResolvedParameter {
        name: Default::default(),
        resolved_type: string_type,
        is_mutable: None,
    };

    let unique_id: ExternalFunctionId = state.allocate_external_function_id();
    let material_png_def = ResolvedExternalFunctionDefinition {
        name: Default::default(),
        signature: ResolvedFunctionSignature {
            first_parameter_is_self: false,
            parameters: (&[mut_self_parameter.clone(), asset_name_parameter.clone()]).to_vec(),
            return_type: ResolvedType::Struct(material_struct_type_ref),
        },
        id: unique_id,
    };

    let _material_png_fn = assets_type
        .borrow_mut()
        .add_external_member_function("material_png", material_png_def.into())?;

    externals.register_external_function(
        "material_png",
        unique_id,
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

    let size_param = ResolvedParameter {
        name: Default::default(),
        resolved_type: ResolvedType::Tuple(size_int_tuple_type_ref),
        is_mutable: None,
    };

    /*
        let _material_png_fn = namespace.util_add_member_external_function(
            &assets_general_type,

        )?;
    */

    let frame_fixed_grid_material_png_def = ResolvedExternalFunctionDefinition {
        name: Default::default(),
        signature: ResolvedFunctionSignature {
            first_parameter_is_self: false,
            parameters: (&[
                mut_self_parameter,
                asset_name_parameter,
                size_param.clone(),
                size_param,
            ])
                .to_vec(),
            return_type: ResolvedType::Struct(fixed_grid_struct_type_ref),
        },
        id: unique_id,
    };

    let _frame_fixed_grid_material_png_fn = assets_type.borrow_mut().add_external_member_function(
        "frame_fixed_grid_material_png",
        frame_fixed_grid_material_png_def.into(),
    )?;

    externals.register_external_function(
        "frame_fixed_grid_material_png",
        unique_id,
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
    gfx_struct_ref: ValueRef,
}

impl ScriptRender {
    pub fn new(
        //resource_storage: &mut ResourceStorage,
        render_value_ref: ValueRef,
        render_struct_type_ref: ResolvedStructTypeRef,
        externals: ExternalFunctions<ScriptRenderContext>,
        gfx_struct_ref: ValueRef,
    ) -> Result<Self, MangroveError> {
        let render_fn = get_impl_func(&render_struct_type_ref, "render");

        Ok(Self {
            render_fn,
            render_value_ref,
            gfx_struct_ref,
            externals,
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

        //info!(render_value=?self.render_value_ref, "render()");

        let self_mut_ref = VariableValue::Reference(ValueReference(self.render_value_ref.clone()));

        util_execute_function(
            &self.externals,
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

    let material_handle_struct_id = resolved_program.state.allocate_number();
    let material_handle_struct_ref = mangrove_render_module
        .namespace
        .borrow_mut()
        .add_generated_struct(
            "MaterialHandle",
            &[(
                "hidden",
                ResolvedType::RustType(material_handle_rust_type_ref.clone()),
            )],
            material_handle_struct_id,
        )?;

    let fixed_atlas_handle_rust_type_ref = Rc::new(ResolvedRustType {
        type_name: "FixedAtlasHandle".to_string(),
        number: 92,
    });

    let fixed_atlas_handle_struct_id = resolved_program.state.allocate_number();

    let fixed_atlas_handle_struct_ref = mangrove_render_module
        .namespace
        .borrow_mut()
        .add_generated_struct(
            "FixedAtlasHandle",
            &[(
                "hidden",
                ResolvedType::RustType(fixed_atlas_handle_rust_type_ref.clone()),
            )],
            fixed_atlas_handle_struct_id,
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

    let render_struct_value = util_execute_function(
        &external_functions,
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
        gfx_value,
    )
}

pub fn render_tick(
    mut script: LoReM<ScriptRender>,
    logic: LoRe<ScriptLogic>,
    mut wgpu_render: ReM<Render>,
) {
    script
        .render(&mut wgpu_render, &logic.immutable_logic_value())
        .expect("script.render() crashed");
}

pub fn detect_reload_tick(
    script_messages: Msg<ScriptMessage>,
    mut script_render: LoReM<ScriptRender>,
    script_logic: LoRe<ScriptLogic>, // it is important that reload is done in correct order, so the new logic is here
    mut all_resources: ReAll,
) {
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => match boot(&mut all_resources, &script_logic.main_module()) {
                Ok(new_render) => *script_render = new_render,
                Err(mangrove_error) => {
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
        let script_render = {
            let script_logic_module = {
                let logic = app.local_resources().fetch::<ScriptLogic>();
                logic.main_module().clone()
            };

            boot(app.resources_mut(), &script_logic_module).expect("could not boot script render")
        };

        app.insert_local_resource(script_render);
        app.add_system(UpdatePhase::Update, detect_reload_tick);
        app.add_system(UpdatePhase::Update, render_tick);
    }
}
