use std::cell::RefCell;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{fs, io};
use swamp::prelude::*;
use swamp_script::prelude::*;
use swamp_script::{compile_and_resolve, ScriptResolveError};
use swamp_script_parser::AstParser;
use swamp_script_semantic::ResolvedImplMemberRef;

#[derive(Debug)]
pub enum MangroveError {
    IoError(std::io::Error),
    ParseError(pest::error::Error<Rule>), // TODO: pest should not leak through here
    ExecuteError(ExecuteError),
    Other(String),
    ScriptResolveError(ScriptResolveError),
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

fn register_print(interpreter: &mut Interpreter) {
    interpreter
        .register_external_function(
            "print".to_string(),
            1, /* TODO: HARD CODED */
            move |args: &[Value]| {
                if let Some(value) = args.first() {
                    let display_value = value.to_string();
                    println!("{}", display_value);
                    Ok(Value::Unit)
                } else {
                    Err("print requires at least one argument".to_string())?
                }
            },
        )
        .expect("should work to register");
}

fn compile(path: &Path) -> Result<ResolvedProgram, MangroveError> {
    let parser = AstParser::new();

    let path_buf = resolve_swamp_file(Path::new(path))?;

    let main_swamp = fs::read_to_string(&path_buf)?;

    let ast_module = parser.parse_script(&main_swamp)?;

    trace!("ast_program:\n{:#?}", ast_module);

    let mut parse_module = ParseModule { ast_module };

    parse_module.declare_external_function(
        "print".to_string(),
        vec![Parameter {
            variable: Variable {
                name: "data".to_string(),
                is_mutable: false,
            },
            param_type: Type::Any,
            is_mutable: false,
        }],
        Type::Unit,
    );

    let root_module_path = ModulePath(vec!["main".to_string()]);
    let resolved_program = compile_and_resolve(path, &root_module_path, parse_module)?;
    Ok(resolved_program)
}

pub struct MangroveApp {
    #[allow(unused)]
    main_program: swamp_script::prelude::ResolvedProgram,
    script_app: Value,
    tick_fn: ResolvedImplMemberRef,
    interpreter: Interpreter,
}

impl Application for MangroveApp {
    fn new(_assets: &mut impl Assets) -> Self {
        let whole_program = compile(Path::new("main.swamp")).expect("Failed to compile main.swamp");
        let main_module = whole_program
            .modules
            .get(&ModulePath(vec!["main".to_string()]))
            .expect("Failed to find main module");

        let mut interpreter = Interpreter::new();

        register_print(&mut interpreter);

        let _script_value_with_signal = interpreter
            .eval_module(&main_module)
            .expect("TODO: panic message");

        let main_fn = main_module
            .namespace
            .get_internal_function("main")
            .expect("No main function");

        let script_app = interpreter
            .util_execute_function(&main_fn, &[])
            .expect("should work");

        let struct_type_ref = match script_app {
            Value::Struct(ref struct_type, _) => struct_type,
            _ => panic!("only support struct for now"),
        };

        let mutable_reference = Value::Reference(Rc::new(RefCell::new(script_app.clone())));

        let binding = struct_type_ref.borrow();
        let tick_fn = binding
            .impl_members
            .get(&IdentifierName("tick".to_string()))
            .expect("must have tick");

        MangroveApp {
            main_program: whole_program,
            script_app: mutable_reference,
            interpreter,
            tick_fn: tick_fn.clone(),
        }
    }

    fn tick(&mut self, _assets: &mut impl Assets) {
        self.interpreter
            .util_execute_member(&self.tick_fn, &[self.script_app.clone()])
            .expect("should work");
    }

    fn render(&mut self, _gfx: &mut impl Gfx) {}
}

fn main() {
    run::<MangroveApp>(
        "mangrove",
        UVec2::new(640, 480),
        UVec2::new(640 * 2, 480 * 2),
    );
}
