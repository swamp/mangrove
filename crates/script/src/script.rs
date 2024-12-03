use monotonic_time_rs::{InstantMonotonicClock, Millis, MonotonicClock};
use seq_map::SeqMapError;
use std::cell::RefCell;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{fs, io};
use swamp::prelude::Assets;
use swamp::prelude::GameAssets;
use swamp::prelude::LocalResource;
use swamp::prelude::ResourceStorage;
use swamp_script::prelude::{
    parse_dependant_modules_and_resolve, DepLoaderError, DependencyParser, IdentifierName,
    ModulePath, Parameter, ParseModule, ResolveError, Type, Variable,
};
use swamp_script::ScriptResolveError;
use swamp_script_eval::prelude::Value;
use swamp_script_eval::{ExecuteError, Interpreter};
use swamp_script_eval_loader::resolve_program;
use swamp_script_parser::{AstParser, Rule};
use swamp_script_semantic::ns::{ResolvedModuleNamespace, SemanticError};
use swamp_script_semantic::{
    ExternalFunctionId, ResolvedModule, ResolvedParameter, ResolvedProgram, ResolvedType,
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

pub struct ScriptIntegration {
    pub assets_material_png_id: ExternalFunctionId,
    assets_value_ref: Value,
}

// I didn't want to change the implementation of GameAssets.
// There might be a better way to do this, but I could not find a way.
// Let's do some pointer magic
pub struct GameAssetsWrapper {
    game_assets: *mut GameAssets<'static>,
}

impl GameAssetsWrapper {
    pub fn new(game_assets: &mut GameAssets) -> Self {
        let ptr = game_assets as *mut GameAssets;
        Self {
            game_assets: ptr as *mut GameAssets<'static>, // Coerce to 'static. is there a better way?
        }
    }

    pub fn material_png(&self, name: &str) -> Value {
        // Safety: We assume the GameAssets pointer is still valid, since the GameAssetsWrapper is short-lived (only alive during a tick)
        let assets: &mut GameAssets;
        unsafe {
            assets = &mut *self.game_assets;
        }
        let something = assets.material_png(name);
        Value::Int(0)
    }
}

pub struct ScriptContext {
    game_assets: GameAssetsWrapper,
}

#[derive(LocalResource)]
pub struct Script {
    clock: InstantMonotonicClock,
    resolved_program: ResolvedProgram,
    interpreter: Interpreter<ScriptContext>,
}

impl Debug for Script {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "script")
    }
}

impl Script {
    pub fn new() -> Self {
        Self {
            clock: InstantMonotonicClock::new(),
            resolved_program: ResolvedProgram::new(),
            interpreter: Interpreter::new(),
        }
    }

    pub fn now(&self) -> Millis {
        self.clock.now()
    }

    pub fn boot(&mut self, resource_storage: &mut ResourceStorage) {
        self.boot_call_main(resource_storage);
    }

    pub fn tick(&mut self) {}

    fn compile(&mut self, path: &Path) -> Result<Value, MangroveError> {
        let parser = AstParser::new();

        let path_buf = resolve_swamp_file(Path::new(path))?;

        let main_swamp = fs::read_to_string(&path_buf)?;

        let ast_module = parser.parse_script(&main_swamp)?;

        trace!("ast_program:\n{:#?}", ast_module);

        let parse_module = ParseModule { ast_module };

        let mut mangrove_module = ResolvedModule::new(ModulePath(vec!["mangrove".to_string()]));
        let asset_struct = self.register_asset_struct(&mut mangrove_module.namespace)?;

        let print_id = self.resolved_program.state.allocate_external_function_id();

        let root_module_path = ModulePath(vec!["main".to_string()]);
        let mut global_module = ResolvedModule::new(root_module_path.clone());

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

        global_module.namespace.util_add_external_function(
            "print",
            print_id,
            &[any_parameter],
            self.resolved_program.types.unit_type(),
        )?;

        self.resolved_program
            .modules
            .modules
            .insert(global_module.module_path.clone(), global_module);

        {
            self.interpreter
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
        }

        self.resolved_program
            .modules
            .modules
            .insert(mangrove_module.module_path.clone(), mangrove_module);

        let mut dependency_parser = DependencyParser::new();
        dependency_parser.add_ast_module(root_module_path.clone(), parse_module);

        let module_paths_in_order = parse_dependant_modules_and_resolve(
            path.to_owned(),
            root_module_path.clone(),
            &mut dependency_parser,
        )?;

        resolve_program(
            &self.resolved_program.types,
            &mut self.resolved_program.state,
            &mut self.resolved_program.modules,
            &module_paths_in_order,
            &dependency_parser,
        )?;

        Ok(asset_struct)
    }

    fn boot_call_main(
        &mut self,
        mut resource_storage: &mut ResourceStorage,
    ) -> Result<(), MangroveError> {
        let assets_value_ref = self
            .compile(Path::new("main.swamp"))
            .expect("Failed to compile main.swamp");

        let main_module = self
            .resolved_program
            .modules
            .get(&ModulePath(vec!["main".to_string()]))
            .expect("Failed to find main module");

        let main_fn = main_module
            .namespace
            .get_internal_function("main")
            .expect("No main function");

        let script_app: Value;
        {
            let mut game_assets = GameAssets::new(&mut resource_storage, Millis::new(0));

            let mut script_context = ScriptContext {
                game_assets: GameAssetsWrapper::new(&mut game_assets),
            };

            script_app = self
                .interpreter
                .util_execute_function(&main_fn, &[assets_value_ref.clone()], &mut script_context)
                .expect("should work");
        }

        let struct_type_ref = match script_app {
            Value::Struct(ref struct_type, _, _) => struct_type,
            _ => panic!("only support struct for now"),
        };

        let mutable_reference = Value::Reference(Rc::new(RefCell::new(script_app.clone())));

        let identifier_name = IdentifierName("tick".to_string());
        let tick_fn = &struct_type_ref
            .borrow()
            .get_internal_member(identifier_name)
            .expect("must have tick");

        Ok(())
    }

    pub fn register_asset_struct(
        &mut self,
        namespace: &mut ResolvedModuleNamespace,
    ) -> Result<Value, MangroveError> {
        let value = 3;
        let (assets_value, assets_type) = Value::new_hidden_rust_type("Assets", value, namespace)?;
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

        let string_type = self.resolved_program.types.string_type();
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

        let unique_id: ExternalFunctionId =
            self.resolved_program.state.allocate_external_function_id();

        let _material_png_fn = namespace.util_add_member_external_function(
            &assets_general_type,
            "material_png",
            unique_id,
            &[mut_self_parameter, asset_name_parameter],
            self.resolved_program.types.int_type(),
        )?;

        self.interpreter.register_external_function(
            "material_png",
            unique_id,
            move |params: &[Value], context| {
                let self_value = &params[0];
                let asset_name = &params[1].expect_string()?;

                println!("material_png called with (self:{self_value} asset_name:'{asset_name}')");

                context.game_assets.material_png(asset_name);

                Ok(Value::Int(42))
            },
        )?;

        Ok(assets_value_mut)
    }
}
