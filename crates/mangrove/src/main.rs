use seq_map::SeqMapError;
use std::any::Any;
use std::cell::RefCell;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{fs, io};

use swamp::prelude::*;
use swamp_script::prelude::*;
use swamp_script::ScriptResolveError;
use swamp_script_eval::value::RustType;
use swamp_script_eval_loader::resolve_program;
use swamp_script_parser::AstParser;
use swamp_script_semantic::ns::SemanticError;

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

// In the future we want to support directories with a project swamp.toml, but for now
// just resolve to single .swamp file
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


pub struct ScriptIntegration {
    pub assets_material_png_id: ExternalFunctionId,
    assets_value_ref: Value,
}

impl ScriptIntegration {
    pub fn new() -> Self {
        Self {
            assets_material_png_id: 0,
            assets_value_ref: Value::Unit,
        }
    }

    fn compile(
        &mut self,
        path: &Path,
        interpreter: &mut Interpreter,
        script_assets: ScriptAssets,
    ) -> Result<ResolvedProgram, MangroveError> {
        let parser = AstParser::new();

        let path_buf = resolve_swamp_file(Path::new(path))?;

        let main_swamp = fs::read_to_string(&path_buf)?;

        let ast_module = parser.parse_script(&main_swamp)?;

        trace!("ast_program:\n{:#?}", ast_module);

        let parse_module = ParseModule { ast_module };

        let mut mangrove_module = ResolvedModule::new(ModulePath(vec!["mangrove".to_string()]));

        let mut resolved_program = ResolvedProgram::new();

        let print_id = resolved_program.state.allocate_external_function_id();

        let global_module_path = ModulePath(vec!["main".to_string()]);
        let mut global_module = ResolvedModule::new(global_module_path);

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
            resolved_program.types.unit_type(),
        )?;

        resolved_program
            .modules
            .modules
            .insert(global_module.module_path.clone(), global_module);

        {
            let namespace = &mut mangrove_module.namespace;

            let (assets_value, assets_type) =
                Value::new_hidden_rust_type("Assets", script_assets, namespace)?;
            self.assets_value_ref = Value::Reference(Rc::new(RefCell::new(assets_value.clone())));

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

            let string_type = resolved_program.types.string_type();
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
                resolved_program.state.allocate_external_function_id();

            let _material_png_fn = namespace.util_add_member_external_function(
                &assets_general_type,
                "material_png",
                unique_id,
                &[mut_self_parameter, asset_name_parameter],
                resolved_program.types.int_type(),
            )?;

            interpreter.register_external_function(
                "material_png",
                unique_id,
                move |params: &[Value]| {
                    let self_value = &params[0];
                    let asset_name = &params[1];
                    println!(
                        "material_png called with (self:{self_value} asset_name:'{asset_name}')"
                    );
                    Ok(Value::Int(42))
                },
            )?;

            interpreter
                .register_external_function("print", print_id, move |args: &[Value]| {
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

        resolved_program
            .modules
            .modules
            .insert(mangrove_module.module_path.clone(), mangrove_module);

        let root_module_path = ModulePath(vec!["main".to_string()]);

        let mut dependency_parser = DependencyParser::new();
        dependency_parser.add_ast_module(root_module_path.clone(), parse_module);

        let module_paths_in_order = parse_dependant_modules_and_resolve(
            path.to_owned(),
            root_module_path.clone(),
            &mut dependency_parser,
        )?;

        resolve_program(
            &mut resolved_program,
            &module_paths_in_order,
            &dependency_parser,
        )?;

        Ok(resolved_program)
    }
}

pub struct MangroveApp {
    #[allow(unused)]
    //main_program: swamp_script::prelude::ResolvedProgram,
    script_app: Value,
    tick_fn: ResolvedInternalFunctionDefinitionRef,
    interpreter: Interpreter,
    #[allow(unused)]
    script_integration: ScriptIntegration,
}

#[derive(Debug)]
pub struct ScriptAssets {
    pub x: i32,
}

impl InternalApplication for MangroveApp {
    fn new(assets: &mut GameAssets) -> Self {
        let mut script_integration = ScriptIntegration::new();

        let mut interpreter = Interpreter::new();

        let script_assets = ScriptAssets {
            x: 32,
        };


        let whole_program = script_integration
            .compile(Path::new("main.swamp"), &mut interpreter, script_assets)
            .expect("Failed to compile main.swamp");

        let main_module = whole_program
            .modules
            .get(&ModulePath(vec!["main".to_string()]))
            .expect("Failed to find main module");

        let _script_value_with_signal = interpreter
            .eval_module(&main_module)
            .expect("TODO: panic message");

        let main_fn = main_module
            .namespace
            .get_internal_function("main")
            .expect("No main function");

        let script_app = interpreter
            .util_execute_function(&main_fn, &[script_integration.assets_value_ref.clone()])
            .expect("should work");

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

        MangroveApp {
            //main_program: whole_program,
            script_app: mutable_reference,
            interpreter,
            tick_fn: tick_fn.clone(),
            script_integration,
        }
    }

    /*
    fn tick(&mut self, _assets: &mut impl Assets) {
        self.interpreter
            .util_execute_member(&self.tick_fn, &[self.script_app.clone()])
            .expect("should work");
    }

    fn render(&mut self, _gfx: &mut impl Gfx) {}

     */
}

fn main() {
    run_internal::<MangroveApp>(
        "mangrove",
        UVec2::new(640, 480),
        UVec2::new(640 * 2, 480 * 2),
    );
}
