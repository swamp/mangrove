use crate::logic::ScriptLogic;
use crate::script::{
    compile, create_empty_struct_value_util, uvec2_like, vec3_like, MangroveError,
};
use crate::util::get_impl_func;
use monotonic_time_rs::Millis;
use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use swamp::prelude::{
    App, Assets, FixedAtlas, FrameLookup, GameAssets, LoRe, LoReM, LocalResource, MaterialRef,
    Plugin, ReM, Render, ResourceStorage, UVec2, UpdatePhase, Vec3,
};
use swamp_script::prelude::{Type, Variable};
use swamp_script_core::prelude::Value;
use swamp_script_eval::prelude::ExecuteError;
use swamp_script_eval::{util_execute_function, ExternalFunctions};
use swamp_script_semantic::prelude::*;

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
        let material_ref_value = Value::RustValue(
            self.material_rust_type_ref.clone(),
            Rc::new(RefCell::new(Box::new(material_ref))),
        );

        Value::Struct(
            self.material_struct_type.clone(),
            [material_ref_value].to_vec(),
            ResolvedType::Struct(self.material_struct_type.clone()),
        )
    }

    fn fixed_atlas_handle(&self, fixed_atlas: FixedAtlas) -> Value {
        let wrapper = FixedAtlasWrapper { fixed_atlas };
        let fixed_atlas_ref = Value::RustValue(
            self.fixed_atlas_rust_type_ref.clone(),
            Rc::new(RefCell::new(Box::new(wrapper))),
        );

        Value::Struct(
            self.fixed_atlas_struct_type_ref.clone(),
            [fixed_atlas_ref].to_vec(),
            ResolvedType::Struct(self.fixed_atlas_struct_type_ref.clone()),
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
) -> Result<Value, MangroveError> {
    let (gfx_value, gfx_struct_type) = create_empty_struct_value_util(namespace, "Gfx")?;
    let gfx_value_mut = Value::Reference(Rc::new(RefCell::new(gfx_value.clone())));
    let assets_general_type = ResolvedType::Struct(gfx_struct_type.clone());

    let mut_self_parameter = ResolvedParameter {
        name: "self".to_string(),
        resolved_type: assets_general_type.clone(),
        ast_parameter: Parameter {
            variable: Variable {
                name: "self".to_string(),
                is_mutable: false,
            },
            param_type: Type::Int,
            is_mutable: true,
            is_self: true,
        },
        is_mutable: true,
    };

    let string_type = types.string_type();
    let material_handle = ResolvedParameter {
        name: "material_handle".to_string(),
        resolved_type: string_type,
        ast_parameter: Parameter {
            variable: Variable {
                name: "".to_string(),
                is_mutable: false,
            },
            param_type: Type::Any,
            is_mutable: false,
            is_self: false,
        },
        is_mutable: false,
    };

    let int_type = types.int_type();
    let tuple_type = ResolvedTupleType(vec![int_type.clone(), int_type.clone(), int_type.clone()]);
    let tuple_type_ref = Rc::new(tuple_type);

    let position_param = ResolvedParameter {
        name: "position".to_string(),
        resolved_type: ResolvedType::Tuple(tuple_type_ref),
        ast_parameter: Parameter {
            variable: Variable {
                name: "".to_string(),
                is_mutable: false,
            },
            param_type: Type::Any,
            is_mutable: false,
            is_self: false,
        },
        is_mutable: false,
    };

    let size_int_tuple_type = ResolvedTupleType(vec![int_type.clone(), int_type.clone()]);
    let size_int_tuple_type_ref = Rc::new(size_int_tuple_type);

    let size_param = ResolvedParameter {
        name: "size".to_string(),
        resolved_type: ResolvedType::Tuple(size_int_tuple_type_ref),
        ast_parameter: Parameter {
            variable: Variable {
                name: "".to_string(),
                is_mutable: false,
            },
            param_type: Type::Int,
            is_mutable: false,
            is_self: false,
        },
        is_mutable: false,
    };

    let unique_id: ExternalFunctionId = state.allocate_external_function_id();

    let _material_png_fn = namespace.util_add_member_external_function(
        &assets_general_type,
        "sprite",
        unique_id,
        &[
            mut_self_parameter.clone(),
            position_param.clone(),
            material_handle.clone(),
            size_param,
        ],
        types.int_type(),
    )?;
    externals.register_external_function(
        "sprite",
        unique_id,
        move |params: &[Value], context| {
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

    let frame = ResolvedParameter {
        name: "frame".to_string(),
        resolved_type: types.int_type(),
        ast_parameter: Parameter {
            variable: Variable {
                name: "".to_string(),
                is_mutable: false,
            },
            param_type: Type::Int,
            is_mutable: false,
            is_self: false,
        },
        is_mutable: false,
    };
    let _material_png_fn = namespace.util_add_member_external_function(
        &assets_general_type,
        "sprite_atlas_frame",
        unique_id,
        &[mut_self_parameter, position_param, material_handle, frame],
        types.int_type(),
    )?;
    externals.register_external_function(
        "sprite_atlas_frame",
        unique_id,
        move |params: &[Value], context| {
            //let _self_value = &params[0]; // the Gfx struct is empty by design.
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
) -> Result<Value, MangroveError> {
    let (assets_value, assets_type) = create_empty_struct_value_util(namespace, "Assets")?;
    let assets_value_mut = Value::Reference(Rc::new(RefCell::new(assets_value.clone())));

    let assets_general_type = ResolvedType::Struct(assets_type.clone());

    let mut_self_parameter = ResolvedParameter {
        name: "self".to_string(),
        resolved_type: assets_general_type.clone(),
        ast_parameter: Parameter {
            variable: Variable {
                name: "self".to_string(),
                is_mutable: false,
            },
            param_type: Type::Int,
            is_mutable: true,
            is_self: true,
        },
        is_mutable: true,
    };

    let string_type = types.string_type();
    let asset_name_parameter = ResolvedParameter {
        name: "asset_name".to_string(),
        resolved_type: string_type,
        ast_parameter: Parameter {
            variable: Variable {
                name: "".to_string(),
                is_mutable: false,
            },
            param_type: Type::String,
            is_mutable: false,
            is_self: false,
        },
        is_mutable: false,
    };

    let unique_id: ExternalFunctionId = state.allocate_external_function_id();

    let _material_png_fn = namespace.util_add_member_external_function(
        &assets_general_type,
        "material_png",
        unique_id,
        &[mut_self_parameter.clone(), asset_name_parameter.clone()],
        ResolvedType::Struct(material_struct_type_ref),
    )?;

    externals.register_external_function(
        "material_png",
        unique_id,
        move |params: &[Value], context| {
            //let self_value = &params[0]; // Assets is, by design, an empty struct
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
        name: "size".to_string(),
        resolved_type: ResolvedType::Tuple(size_int_tuple_type_ref),
        ast_parameter: Parameter {
            variable: Variable {
                name: "".to_string(),
                is_mutable: false,
            },
            param_type: Type::Int,
            is_mutable: false,
            is_self: false,
        },
        is_mutable: false,
    };

    let _material_png_fn = namespace.util_add_member_external_function(
        &assets_general_type,
        "frame_fixed_grid_material_png",
        unique_id,
        &[
            mut_self_parameter,
            asset_name_parameter,
            size_param.clone(),
            size_param,
        ],
        ResolvedType::Struct(fixed_grid_struct_type_ref),
    )?;
    externals.register_external_function(
        "frame_fixed_grid_material_png",
        unique_id,
        move |params: &[Value], context| {
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
    render_value_ref: Value,
    render_fn: ResolvedInternalFunctionDefinitionRef,
    externals: ExternalFunctions<ScriptRenderContext>,
    gfx_struct_value: Value,
}

impl ScriptRender {
    pub fn new(
        //resource_storage: &mut ResourceStorage,
        render_value_ref: Value,
        render_struct_type_ref: ResolvedStructTypeRef,
        externals: ExternalFunctions<ScriptRenderContext>,
        gfx_struct_value: Value,
    ) -> Result<Self, MangroveError> {
        let render_fn = get_impl_func(&render_struct_type_ref, "render");

        Ok(Self {
            render_fn,
            render_value_ref,
            gfx_struct_value,
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

        util_execute_function(
            &self.externals,
            &self.render_fn,
            &[
                self.render_value_ref.clone(),
                logic_value_ref.clone(),
                self.gfx_struct_value.clone(),
            ]
            .to_vec(),
            &mut script_context,
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
        Value,
        Value,
        ResolvedStructTypeRef,
        ResolvedRustTypeRef,
        ResolvedStructTypeRef,
        ResolvedRustTypeRef,
    ),
    MangroveError,
> {
    let mut mangrove_render_module = ResolvedModule::new(ModulePath(vec![
        "mangrove".to_string(),
        "render".to_string(),
    ]));

    let material_handle_rust_type_ref = Rc::new(ResolvedRustType {
        type_name: "MaterialHandle".to_string(),
        number: 91,
    });
    let material_handle_struct_ref = mangrove_render_module.namespace.util_insert_struct_type(
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
    let fixed_atlas_handle_struct_ref = mangrove_render_module.namespace.util_insert_struct_type(
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
        &mut mangrove_render_module.namespace,
        material_handle_struct_ref.clone(),
        fixed_atlas_handle_struct_ref.clone(),
    )?;

    let gfx_struct_value = register_gfx_struct_value_with_members(
        &resolved_program.types,
        &mut resolved_program.state,
        &mut externals,
        &mut mangrove_render_module.namespace,
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
    resolved_program.modules.add_module(render_module_ref)?;
    resolved_program.modules.add_linked_module(
        ModulePath(["logic".to_string()].to_vec()),
        logic_main_module.clone(),
    )?;

    compile(
        "scripts/render.swamp".as_ref(),
        &mut resolved_program,
        &mut external_functions,
    )?;

    let root_module_path = ModulePath(vec!["main".to_string()]);
    let main_module = resolved_program
        .modules
        .get(&root_module_path)
        .expect("could not find main module");

    let binding = main_module.borrow();
    let main_fn = binding
        .namespace
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
    )?;

    let render_struct_type_ref =
        if let Value::Struct(struct_type_ref, _, _) = render_struct_value.clone() {
            struct_type_ref
        } else {
            return Err(MangroveError::Other("needs to be logic struct".to_string()));
        };

    // let render_fn = get_impl_func(&render_struct_type_ref, "render");

    // Convert it to a mutable (reference), so it can be mutated in update ticks
    let render_struct_value_mutable_ref =
        Value::Reference(Rc::new(RefCell::new(render_struct_value)));

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

pub struct ScriptRenderPlugin;

impl Plugin for ScriptRenderPlugin {
    fn build(&self, app: &mut App) {
        // Get the module in its own scope
        let script_logic_module = {
            let logic = app.local_resources().fetch::<ScriptLogic>();
            logic.main_module().clone() // Assuming we can clone the module
        }; // immutable borrow is dropped here

        let script_render =
            boot(app.resources_mut(), &script_logic_module).expect("could not boot script render");

        app.insert_local_resource(script_render);
        app.add_system(UpdatePhase::Update, render_tick);
    }
}
