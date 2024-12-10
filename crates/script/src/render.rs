use crate::script::{create_empty_struct_value_util, uvec2_like, vec3_like, MangroveError};
use std::cell::RefCell;
use std::rc::Rc;
use swamp::prelude::{GameAssets, LocalResource, MaterialRef, Render, UVec2, Vec3};
use swamp_script::prelude::{Type, Variable};
use swamp_script_core::prelude::Value;
use swamp_script_eval::ExternalFunctions;
use swamp_script_semantic::prelude::*;

pub struct ScriptRenderContext {
    pub(crate) game_assets: Option<GameAssetsWrapper>,
    pub(crate) render: Option<RenderWrapper>,
}

// I didn't want to change the implementation of GameAssets.
// There might be a better way to do this, but I could not find a way.
// Let's do some pointer magic
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
}

// I didn't want to change the implementation of GameAssets.
// There might be a better way to do this, but I could not find a way.
// Let's do some pointer magic
pub struct GameAssetsWrapper {
    game_assets: *mut GameAssets<'static>,
    material_struct_type: ResolvedStructTypeRef,
    material_rust_type_ref: ResolvedRustTypeRef,
}

impl GameAssetsWrapper {
    pub fn new(
        game_assets: &mut GameAssets,
        material_struct_type: ResolvedStructTypeRef,
        material_rust_type_ref: ResolvedRustTypeRef,
    ) -> Self {
        let ptr = game_assets as *mut GameAssets;
        Self {
            game_assets: ptr as *mut GameAssets<'static>, // Coerce to 'static. is there a better way?
            material_struct_type,
            material_rust_type_ref,
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

    pub fn material_png(&self, name: &str) -> Value {
        // Safety: We assume the GameAssets pointer is still valid, since the GameAssetsWrapper is short-lived (only alive during a tick)
        let assets: &mut GameAssets;
        unsafe {
            assets = &mut *self.game_assets;
        }
        // let material_ref = assets.material_png(name);

        //self.material_handle(material_ref)
        Value::Bool(false)
    }
}

#[derive(LocalResource, Debug)]
pub struct ScriptRenderTypes {
    pub material_handle_struct_ref: ResolvedStructTypeRef,
    assets_struct_value: Value,
    gfx_struct_value: Value,
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
            mut_self_parameter,
            position_param,
            material_handle,
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

    Ok(gfx_value_mut)
}

pub fn register_asset_struct_value_with_members(
    types: &ResolvedProgramTypes,
    state: &mut ResolvedProgramState,
    externals: &mut ExternalFunctions<ScriptRenderContext>,
    namespace: &mut ResolvedModuleNamespace,
    material_struct_type_ref: ResolvedStructTypeRef,
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
        &[mut_self_parameter, asset_name_parameter],
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

    Ok(assets_value_mut)
}
/*




impl ScriptRenderTypes {
    pub fn new() -> Result<(Self, ResolvedModule), SemanticError> {
        let material_handle_rust_type_ref = Rc::new(ResolvedRustType {
            type_name: "MaterialHandle".to_string(),
            number: 91,
        });

        let mut mangrove_render_module = ResolvedModule::new(ModulePath(vec![
            "mangrove".to_string(),
            "render".to_string(),
        ]));
        let material_handle_struct_ref = mangrove_render_module.namespace.util_insert_struct_type(
            "MaterialHandle",
            &[(
                "hidden",
                ResolvedType::RustType(material_handle_rust_type_ref.clone()),
            )],
        )?;

        let assets_struct_value = register_asset_struct_value_with_members(
            &types,
            &mut state,
            &mut externals,
            &mut mangrove_module.namespace,
            material_handle_struct_ref.clone(),
        )?;

        let gfx_struct_value = register_gfx_struct_value_with_members(
            &types,
            &mut state,
            &mut externals,
            &mut mangrove_module.namespace,
        )?;

        Ok((
            Self {
                material_handle_struct_ref,
                assets_struct_value,
                gfx_struct_value,
            },
            mangrove_render_module,
        ))
    }
}

pub struct ScriptRender {
    render_value_ref: Value,
    render_fn: ResolvedInternalFunctionDefinitionRef,
}

impl ScriptRender {
    pub fn init(
        resource_storage: &mut ResourceStorage,
        render_value_ref: Value,
        render_struct_type_ref: ResolvedStructTypeRef,
    ) -> Result<Self, MangroveError> {
        let render_fn = get_impl_func(&render_struct_type_ref, "render");

        Ok(Self {
            render_fn,
            render_value_ref,
        })
    }

    pub fn render(
        &mut self,
        wgpu_render: &mut Render,
        externals: &ExternalFunctions<ScriptContext>,
        logic_value_ref: &Value,
        gfx_struct_value: &Value,
    ) -> Result<(), ExecuteError> {
        let mut script_context = ScriptContext {
            game_assets: None,
            render: Some(RenderWrapper::new(wgpu_render)),
        };

        util_execute_function(
            &externals,
            &self.render_fn,
            &[
                self.render_value_ref.clone(),
                logic_value_ref.clone(),
                gfx_struct_value.clone(),
            ]
            .to_vec(),
            &mut script_context,
        )?;

        Ok(())
    }
}

pub struct ScriptRenderPlugin;

impl Plugin for ScriptRenderPlugin {
    fn build(&self, app: &mut App) {
        let script_render_types = ScriptRenderTypes::new().expect("render types failed");
        app.insert_local_resource(script_render_types);

        let mut resource_modules = app.resource_mut::<ScriptModules>();
        resource_modules
            .modules
            .add_module(script_render_types.render_module);


    }
}
*/
