/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use monotonic_time_rs::{InstantMonotonicClock, Millis, MonotonicClock};
use seq_map::SeqMapError;
use std::cell::RefCell;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{fs, io};
use swamp::prelude::ResourceStorage;
use swamp::prelude::{Assets, MaterialRef};
use swamp::prelude::{GameAssets, Render};
use swamp::prelude::{LocalResource, UVec2, Vec3};
use swamp_script::prelude::{
    parse_dependant_modules_and_resolve, DepLoaderError, DependencyParser, IdentifierName,
    ModulePath, Parameter, ParseModule, ResolveError, Type, Variable,
};
use swamp_script::ScriptResolveError;
use swamp_script_eval::prelude::Value;
use swamp_script_eval::value::ConversionError;
use swamp_script_eval::{util_execute_function, ExecuteError, ExternalFunctions};
use swamp_script_eval_loader::resolve_program;
use swamp_script_parser::{AstParser, Rule};
use swamp_script_semantic::ns::{ResolvedModuleNamespace, SemanticError};
use swamp_script_semantic::{
    ExternalFunctionId, ResolvedInternalFunctionDefinitionRef, ResolvedModule, ResolvedModules,
    ResolvedParameter, ResolvedProgramState, ResolvedProgramTypes, ResolvedStructTypeRef,
    ResolvedTupleType, ResolvedTupleTypeRef, ResolvedType,
};
use tracing::trace;

fn resolve_swamp_file(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err(format!("Path does not exist: {}", path.display()));
    }

    if path.is_dir() {
        let main_file = path.join("main.swamp");
        if !main_file.exists() {
            return Err(format!(
                "No main.swamp found in directory: {}",
                path.display()
            ));
        }
        Ok(main_file)
    } else if path.extension().and_then(|ext| ext.to_str()) == Some("swamp") {
        Ok(path.to_path_buf())
    } else {
        Err(format!("Not a .swamp file: {}", path.display()))
    }
}

#[derive(Debug)]
pub enum MangroveError {
    IoError(std::io::Error),
    ParseError(pest::error::Error<Rule>), // TODO: pest should not leak through here
    ExecuteError(ExecuteError),
    Other(String),
    ScriptResolveError(ScriptResolveError),
    SemanticError(SemanticError),
    ResolveError(ResolveError),
    DepLoaderError(DepLoaderError),
    SeqMapError(SeqMapError),
}

impl Display for MangroveError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for MangroveError {}

impl From<io::Error> for MangroveError {
    fn from(value: io::Error) -> Self {
        Self::IoError(value)
    }
}

impl From<ScriptResolveError> for MangroveError {
    fn from(value: ScriptResolveError) -> Self {
        Self::ScriptResolveError(value)
    }
}
impl From<ExecuteError> for MangroveError {
    fn from(value: ExecuteError) -> Self {
        Self::ExecuteError(value)
    }
}

impl From<SeqMapError> for MangroveError {
    fn from(value: SeqMapError) -> Self {
        Self::SeqMapError(value)
    }
}

impl From<SemanticError> for MangroveError {
    fn from(value: SemanticError) -> Self {
        Self::SemanticError(value)
    }
}

impl From<ResolveError> for MangroveError {
    fn from(value: ResolveError) -> Self {
        Self::ResolveError(value)
    }
}

impl From<DepLoaderError> for MangroveError {
    fn from(value: DepLoaderError) -> Self {
        Self::DepLoaderError(value)
    }
}

impl From<pest::error::Error<Rule>> for MangroveError {
    fn from(value: pest::error::Error<Rule>) -> Self {
        Self::ParseError(value)
    }
}

impl From<String> for MangroveError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
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
}

impl GameAssetsWrapper {
    pub fn new(game_assets: &mut GameAssets, material_struct_type: ResolvedStructTypeRef) -> Self {
        let ptr = game_assets as *mut GameAssets;
        Self {
            game_assets: ptr as *mut GameAssets<'static>, // Coerce to 'static. is there a better way?
            material_struct_type,
        }
    }

    fn material_handle(&self, material_ref: MaterialRef) -> Value {
        let material_ref_value = Value::RustValue(Rc::new(RefCell::new(Box::new(material_ref))));
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
        let material_ref = assets.material_png(name);

        self.material_handle(material_ref)
    }
}

pub struct ScriptContext {
    game_assets: Option<GameAssetsWrapper>,
    render: Option<RenderWrapper>,
}

pub fn create_empty_struct_type(
    namespace: &mut ResolvedModuleNamespace,
    name: &str,
) -> Result<ResolvedStructTypeRef, SemanticError> {
    namespace.util_insert_struct_type(name, &[("hidden", ResolvedType::Any)])
}

pub fn create_empty_struct_value(struct_type: ResolvedStructTypeRef) -> Value {
    Value::Struct(
        struct_type.clone(),
        [].to_vec(),
        ResolvedType::Struct(struct_type.clone()),
    )
}

pub fn create_empty_struct_value_util(
    mut namespace: &mut ResolvedModuleNamespace,
    name: &str,
) -> Result<(Value, ResolvedStructTypeRef), SemanticError> {
    let struct_type = create_empty_struct_type(&mut namespace, name)?;
    Ok((create_empty_struct_value(struct_type.clone()), struct_type))
}

pub fn register_asset_struct_value_with_members(
    types: &ResolvedProgramTypes,
    state: &mut ResolvedProgramState,
    externals: &mut ExternalFunctions<ScriptContext>,
    namespace: &mut ResolvedModuleNamespace,
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
        types.int_type(),
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

fn vec3_like(v: &Value) -> Result<Vec3, ConversionError> {
    match v {
        Value::Tuple(_, fields) => {
            let x = fields[0].expect_int()?;
            let y = fields[1].expect_int()?;
            let z = fields[2].expect_int()?;

            Ok(Vec3::new(x as i16, y as i16, z as i16))
        }
        _ => Err(ConversionError::ValueError("not a vec3".to_string())),
    }
}

fn uvec2_like(v: &Value) -> Result<UVec2, ConversionError> {
    match v {
        Value::Tuple(_, fields) => {
            let width = fields[0].expect_int()?;
            let height = fields[1].expect_int()?;

            Ok(UVec2::new(width as u16, height as u16))
        }
        _ => Err(ConversionError::ValueError("not a vec3".to_string())),
    }
}

pub fn register_gfx_struct_value_with_members(
    types: &ResolvedProgramTypes,
    state: &mut ResolvedProgramState,
    externals: &mut ExternalFunctions<ScriptContext>,
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

fn create_mangrove_module(
    types: &ResolvedProgramTypes,
    mut state: &mut ResolvedProgramState,
    mut externals: &mut ExternalFunctions<ScriptContext>,
) -> Result<(ResolvedModule, Value, Value, ResolvedStructTypeRef), MangroveError> {
    let mut mangrove_module = ResolvedModule::new(ModulePath(vec!["mangrove".to_string()]));

    let assets_struct_value = register_asset_struct_value_with_members(
        &types,
        &mut state,
        &mut externals,
        &mut mangrove_module.namespace,
    )?;

    let gfx_struct_value = register_gfx_struct_value_with_members(
        &types,
        &mut state,
        &mut externals,
        &mut mangrove_module.namespace,
    )?;

    let material_handle_struct_ref = mangrove_module
        .namespace
        .util_insert_struct_type("MaterialHandle", &[("hidden", ResolvedType::Any)])?;

    Ok((
        mangrove_module,
        assets_struct_value,
        gfx_struct_value,
        material_handle_struct_ref,
    ))
}

fn prepare_main_module(
    types: &ResolvedProgramTypes,
    state: &mut ResolvedProgramState,
    externals: &mut ExternalFunctions<ScriptContext>,
) -> Result<ResolvedModule, SemanticError> {
    let root_module_path = ModulePath(vec!["main".to_string()]);
    let mut main_module = ResolvedModule::new(root_module_path.clone());

    let any_parameter = ResolvedParameter {
        name: "data".to_string(),
        resolved_type: ResolvedType::Any,
        ast_parameter: Parameter {
            variable: Variable {
                name: "data".to_string(),
                is_mutable: false,
            },
            param_type: Type::Any,
            is_mutable: false,
            is_self: false,
        },
        is_mutable: false,
    };

    let print_id = state.allocate_external_function_id();

    main_module.namespace.util_add_external_function(
        "print",
        print_id,
        &[any_parameter],
        types.unit_type(),
    )?;

    externals
        .register_external_function("print", print_id, move |args: &[Value], _| {
            if let Some(value) = args.first() {
                let display_value = value.to_string();
                println!("{}", display_value);
                Ok(Value::Unit)
            } else {
                Err("print requires at least one argument".to_string())?
            }
        })
        .expect("should work to register");

    Ok(main_module)
}

fn parse_module(path: PathBuf) -> Result<ParseModule, MangroveError> {
    let parser = AstParser::new();

    let path_buf = resolve_swamp_file(Path::new(&path))?;

    let main_swamp = fs::read_to_string(&path_buf)?;

    let ast_module = parser.parse_script(&main_swamp)?;

    trace!("ast_program:\n{:#?}", ast_module);

    let parse_module = ParseModule { ast_module };

    Ok(parse_module)
}

fn compile(
    path: &Path,
    types: &ResolvedProgramTypes,
    mut state: &mut ResolvedProgramState,
    externals: &mut ExternalFunctions<ScriptContext>,
    mut modules: &mut ResolvedModules,
) -> Result<(Value, Value, ResolvedStructTypeRef), MangroveError> {
    let parsed_module = parse_module(PathBuf::from(path))?;

    let (mangrove_module, assets_value, gfx_value, material_struct_ref) =
        create_mangrove_module(types, state, externals)?;
    let main_module = prepare_main_module(types, state, externals)?;
    let main_path = main_module.module_path.clone();
    modules.add_module(main_module)?;
    modules.add_module(mangrove_module)?;

    let mut dependency_parser = DependencyParser::new();
    dependency_parser.add_ast_module(main_path.clone(), parsed_module);

    let module_paths_in_order = parse_dependant_modules_and_resolve(
        path.to_owned(),
        main_path.clone(),
        &mut dependency_parser,
    )?;

    resolve_program(
        &types,
        &mut state,
        &mut modules,
        &module_paths_in_order,
        &dependency_parser,
    )?;

    Ok((assets_value, gfx_value, material_struct_ref))
}

fn boot_call_main(
    main_module: &ResolvedModule,
    mut resource_storage: &mut ResourceStorage,
    material_handle_struct_ref: ResolvedStructTypeRef,
    externals: &ExternalFunctions<ScriptContext>,
    assets_value_mut: Value,
) -> Result<
    (
        Value,
        ResolvedInternalFunctionDefinitionRef,
        Value,
        ResolvedInternalFunctionDefinitionRef,
        Value,
        ResolvedInternalFunctionDefinitionRef,
    ),
    MangroveError,
> {
    let main_fn = main_module
        .namespace
        .get_internal_function("main")
        .expect("No main function");

    let script_app_tuple: Value;
    {
        let mut game_assets = GameAssets::new(&mut resource_storage, Millis::new(0));

        let mut script_context = ScriptContext {
            game_assets: Some(GameAssetsWrapper::new(
                &mut game_assets,
                material_handle_struct_ref.clone(),
            )),
            render: None,
        };

        script_app_tuple = util_execute_function(
            &externals,
            &main_fn,
            &[assets_value_mut.clone()],
            &mut script_context,
        )
        .expect("should work");
    }

    let (tuple_type, fields) = match script_app_tuple {
        Value::Tuple(ref tuple_type, fields) => (tuple_type, fields),
        _ => panic!("only support struct for now"),
    };

    // Use references so they can be mutated
    let logic_value_ref = Value::Reference(Rc::new(RefCell::new(fields[0].clone())));
    let render_value_ref = Value::Reference(Rc::new(RefCell::new(fields[1].clone())));
    let audio_value_ref = Value::Reference(Rc::new(RefCell::new(fields[2].clone())));

    let tuple_types = &tuple_type.0;
    let logic_struct_type = tuple_types[0].expect_struct_type()?;
    let render_struct_type = tuple_types[1].expect_struct_type()?;
    let audio_struct_type = tuple_types[2].expect_struct_type()?;

    let identifier_name = IdentifierName("tick".to_string());
    let logic_fn = &logic_struct_type
        .borrow()
        .get_internal_member(identifier_name)
        .expect("must have tick");

    let identifier_name = IdentifierName("render".to_string());
    let render_fn = &render_struct_type
        .borrow()
        .get_internal_member(identifier_name)
        .expect("must have render");

    let identifier_name = IdentifierName("audio".to_string());
    let audio_fn = &audio_struct_type
        .borrow()
        .get_internal_member(identifier_name)
        .expect("must have audio");

    Ok((
        logic_value_ref,
        logic_fn.clone(),
        render_value_ref,
        render_fn.clone(),
        audio_value_ref,
        audio_fn.clone(),
    ))
}

#[derive(LocalResource)]
pub struct Script {
    clock: InstantMonotonicClock,
    externals: ExternalFunctions<ScriptContext>,
    //material_handle_struct_ref: ResolvedStructTypeRef,
    gfx_struct_value: Value,
    render_fn: ResolvedInternalFunctionDefinitionRef,
    logic_fn: ResolvedInternalFunctionDefinitionRef,
    #[allow(unused)]
    audio_fn: ResolvedInternalFunctionDefinitionRef,

    // Script state that is kept alive
    logic_value_ref: Value,
    render_value_ref: Value,
    #[allow(unused)]
    audio_value_ref: Value,
}

impl Debug for Script {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "script")
    }
}

impl Script {
    pub fn new(resource_storage: &mut ResourceStorage) -> Result<Self, MangroveError> {
        let mut state = ResolvedProgramState::new();
        let mut modules = ResolvedModules::new();
        let types = ResolvedProgramTypes::new();
        let mut externals = ExternalFunctions::<ScriptContext>::new();

        let (assets_value_mut, gfx_value_mut, material_struct_ref) = compile(
            Path::new("scripts/main.swamp"),
            &types,
            &mut state,
            &mut externals,
            &mut modules,
        )?;

        let main_module = modules
            .get(&ModulePath(vec!["main".to_string()]))
            .expect("Failed to find main module");

        let (logic_value_mut, logic_fn, render_value_mut, render_fn, audio_value_mut, audio_fn) =
            boot_call_main(
                main_module,
                resource_storage,
                material_struct_ref.clone(),
                &externals,
                assets_value_mut,
            )?;

        Ok(Self {
            clock: InstantMonotonicClock::new(),
            externals,
            gfx_struct_value: gfx_value_mut,
            render_fn,
            logic_fn,
            audio_fn,
            logic_value_ref: logic_value_mut,
            render_value_ref: render_value_mut,
            audio_value_ref: audio_value_mut,
        })
    }

    pub fn now(&self) -> Millis {
        self.clock.now()
    }

    pub fn tick(&mut self) -> Result<(), ExecuteError> {
        let mut script_context = ScriptContext {
            game_assets: None,
            render: None,
        };

        util_execute_function(
            &self.externals,
            &self.logic_fn,
            &[self.logic_value_ref.clone()].to_vec(),
            &mut script_context,
        )?;

        Ok(())
    }

    pub fn render(&mut self, wgpu_render: &mut Render) -> Result<(), ExecuteError> {
        let mut script_context = ScriptContext {
            game_assets: None,
            render: Some(RenderWrapper::new(wgpu_render)),
        };

        util_execute_function(
            &self.externals,
            &self.render_fn,
            &[
                self.render_value_ref.clone(),
                self.logic_value_ref.clone(),
                self.gfx_struct_value.clone(),
            ]
            .to_vec(),
            &mut script_context,
        )?;

        Ok(())
    }
}
