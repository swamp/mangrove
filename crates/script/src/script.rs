/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use seq_map::SeqMapError;
use std::cell::RefCell;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{fs, io};
use swamp::prelude::{UVec2, Vec3};
use swamp_script::prelude::{
    parse_dependant_modules_and_resolve, DepLoaderError, DependencyParser, ModulePath, Parameter,
    ParseModule, ResolveError, Type, Variable,
};
use swamp_script::ScriptResolveError;
use swamp_script_core::prelude::Value;
use swamp_script_core::value::ValueError;
use swamp_script_eval::prelude::*;
use swamp_script_eval_loader::resolve_program;
use swamp_script_parser::{AstParser, Rule};
use swamp_script_semantic::prelude::*;
use swamp_script_std::create_std_module;
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

pub fn vec3_like(v: &Value) -> Result<Vec3, ValueError> {
    match v {
        Value::Tuple(_, fields) => {
            let x = fields[0].expect_int()?;
            let y = fields[1].expect_int()?;
            let z = fields[2].expect_int()?;

            Ok(Vec3::new(x as i16, y as i16, z as i16))
        }
        _ => Err(ValueError::TypeError("not a vec3".to_string())),
    }
}

pub fn uvec2_like(v: &Value) -> Result<UVec2, ValueError> {
    match v {
        Value::Tuple(_, fields) => {
            let width = fields[0].expect_int()?;
            let height = fields[1].expect_int()?;

            Ok(UVec2::new(width as u16, height as u16))
        }
        _ => Err(ValueError::TypeError("not a vec3".to_string())),
    }
}

fn prepare_main_module<C>(
    types: &ResolvedProgramTypes,
    state: &mut ResolvedProgramState,
    externals: &mut ExternalFunctions<C>,
    module_name: &str,
) -> Result<ResolvedModule, SemanticError> {
    let root_module_path = ModulePath(vec![module_name.to_string()]);
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
                let display_value = value.convert_to_string_if_needed();
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

pub fn compile<C>(
    path: &Path,
    resolved_program: &mut ResolvedProgram,
    externals: &mut ExternalFunctions<C>,
    module_name: &str,
) -> Result<(), MangroveError> {
    let parsed_module = parse_module(PathBuf::from(path))?;

    let main_module = prepare_main_module(
        &resolved_program.types,
        &mut resolved_program.state,
        externals,
        module_name,
    )?;

    let main_path = main_module.module_path.clone();

    let main_module_ref = Rc::new(RefCell::new(main_module));
    resolved_program.modules.add_module(main_module_ref)?;

    resolved_program
        .modules
        .add_module(Rc::new(RefCell::new(create_std_module())));

    let mut dependency_parser = DependencyParser::new();
    dependency_parser.add_ast_module(main_path.clone(), parsed_module);

    let module_paths_in_order = parse_dependant_modules_and_resolve(
        path.to_owned(),
        main_path.clone(),
        &mut dependency_parser,
    )?;

    resolve_program(
        &resolved_program.types,
        &mut resolved_program.state,
        &mut resolved_program.modules,
        &module_paths_in_order,
        &dependency_parser,
    )?;

    Ok(())
}

/*
fn boot_call_main(
    main_module: &ResolvedModule,
    mut resource_storage: &mut ResourceStorage,
    material_handle_struct_ref: ResolvedStructTypeRef,
    material_handle_rust_type_ref: ResolvedRustTypeRef,
    externals: &ExternalFunctions<ScriptContext>,
    assets_value_mut: Value,
) -> Result<
    (
        Value,
        ResolvedStructTypeRef,
        Value,
        ResolvedStructTypeRef,
        Value,
        ResolvedStructTypeRef,
    ),
    MangroveError,
> {
    let main_fn = main_module
        .namespace
        .get_internal_function("main")
        .expect("No main function");

    let script_app_tuple: Value;
    {




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

    let render_value_ref = Value::Reference(Rc::new(RefCell::new(fields[1].clone())));
    let audio_value_ref = Value::Reference(Rc::new(RefCell::new(fields[2].clone())));

    let tuple_types = &tuple_type.0;
    let logic_struct_type = tuple_types[0].expect_struct_type()?;
    let render_struct_type = tuple_types[1].expect_struct_type()?;
    let audio_struct_type = tuple_types[2].expect_struct_type()?;

    Ok((
        logic_value_ref,
        logic_struct_type.clone(),
        render_value_ref,
        render_struct_type.clone(),
        audio_value_ref,
        audio_struct_type.clone(),
    ))
}

#[derive(LocalResource)]
pub struct Script {
    clock: InstantMonotonicClock,
    externals: ExternalFunctions<ScriptContext>,


    // Audio
    #[allow(unused)]
    audio_value_ref: Value,
    #[allow(unused)]
    audio_fn: ResolvedInternalFunctionDefinitionRef,
}

impl Debug for Script {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "script")
    }
}

 */
/*
impl Script {
    pub fn new(resource_storage: &mut ResourceStorage) -> Result<Self, MangroveError> {
        let mut state = ResolvedProgramState::new();
        let mut modules = ResolvedModules::new();
        let types = ResolvedProgramTypes::new();
        let mut externals = ExternalFunctions::<ScriptContext>::new();



        compile(
            Path::new("scripts/main.swamp"),
            &types,
            &mut state,
            &mut externals,
            &mut modules,
        )?;

        let main_module = modules
            .get(&ModulePath(vec!["main".to_string()]))
            .expect("Failed to find main module");

        let (
            logic_value_mut,
            logic_struct_type,
            render_value_mut,
            render_struct_type,
            audio_value_mut,
            audio_struct_type,
        ) = boot_call_main(
            main_module,
            resource_storage,
            material_struct_ref.clone(),
            material_handle_rust_type_ref,
            &externals,
            assets_value_mut,
        )?;


        Ok(Self {
            clock: InstantMonotonicClock::new(),
            externals,
            gfx_struct_value: gfx_value_mut,
            render_fn,
            logic_fn,
            gamepad_changed_fn,
            audio_fn,
            logic_value_ref: logic_value_mut,
            render_value_ref: render_value_mut,
            audio_value_ref: audio_value_mut,
        })
    }

    pub fn now(&self) -> Millis {
        self.clock.now()
    }

}
*/
