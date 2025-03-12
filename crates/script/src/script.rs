/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use std::cell::RefCell;
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;
use std::time::Instant;
use swamp::prelude::{Color, Rotation, SpriteParams, UVec2, Vec2, Vec3};
use swamp_script::compile_and_analyze;
use swamp_script::prelude::*;
use yansi::Paint;

#[derive(Debug)]
pub enum MangroveError {
    RuntimeError(RuntimeError),
    Other(String),
    ScriptResolveError(ScriptResolveError),
}

impl Display for MangroveError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<ScriptResolveError> for MangroveError {
    fn from(value: ScriptResolveError) -> Self {
        Self::ScriptResolveError(value)
    }
}
impl From<RuntimeError> for MangroveError {
    fn from(value: RuntimeError) -> Self {
        Self::RuntimeError(value)
    }
}

impl From<String> for MangroveError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

pub fn create_empty_struct_type(
    symbol_table: &mut SymbolTable,
    name: &str,
) -> Result<NamedStructType, Error> {
    Ok(symbol_table.add_generated_struct(name, &[("hidden", Type::Unit)])?)
}

pub fn create_empty_struct_value(struct_type: NamedStructType) -> Value {
    Value::NamedStruct(struct_type, [].to_vec())
}

pub fn create_empty_struct_value_util(
    symbol_table: &mut SymbolTable,
    name: &str,
) -> Result<(Value, NamedStructType), Error> {
    let struct_type = create_empty_struct_type(symbol_table, name)?;
    Ok((create_empty_struct_value(struct_type.clone()), struct_type))
}

pub fn sprite_params(sprite_params_struct: &Value) -> Result<SpriteParams, ValueError> {
    if let Value::NamedStruct(_struct_type_ref, fields) = sprite_params_struct {
        Ok(SpriteParams {
            scale: fields[4].borrow().expect_int()? as u8,
            texture_size: uvec2_like(&fields[6].borrow())?,
            texture_pos: uvec2_like(&fields[5].borrow())?,
            flip_x: fields[0].borrow().as_bool()?,
            flip_y: fields[1].borrow().as_bool()?,
            rotation: match fields[2].borrow().expect_int()? % 4 {
                0 => Rotation::Degrees0,
                1 => Rotation::Degrees90,
                2 => Rotation::Degrees180,
                3 => Rotation::Degrees270,
                _ => return Err(ValueError::TypeError("wrong rotation".to_string())),
            },
            pivot: Vec2::new(0, 0),
            color: color_like(&fields[3].borrow())?,
        })
    } else {
        Err(ValueError::TypeError("not a sprite param".to_string()))
    }
}

pub fn value_to_value_ref(fields: &[Value]) -> Vec<ValueRef> {
    fields
        .iter()
        .map(|v| Rc::new(RefCell::new(v.clone())))
        .clone()
        .collect()
}

pub fn vec3_like(v: &Value) -> Result<Vec3, ValueError> {
    match v {
        Value::Tuple(_, fields) => {
            let x = fields[0].borrow().expect_int()?;
            let y = fields[1].borrow().expect_int()?;
            let z = fields[2].borrow().expect_int()?;

            Ok(Vec3::new(x as i16, y as i16, z as i16))
        }
        _ => Err(ValueError::TypeError("not a vec3".to_string())),
    }
}

pub fn color_like(v: &Value) -> Result<Color, ValueError> {
    match v {
        Value::NamedStruct(_, fields) => {
            let r = fields[0].borrow().expect_float()?;
            let g = fields[1].borrow().expect_float()?;
            let b = fields[2].borrow().expect_float()?;
            let a = fields[3].borrow().expect_float()?;

            Ok(Color::from_f32(r.into(), g.into(), b.into(), a.into()))
        }
        _ => Err(ValueError::TypeError("not a color".to_string())),
    }
}

pub fn uvec2_like(v: &Value) -> Result<UVec2, ValueError> {
    match v {
        Value::Tuple(_, fields) => {
            let width = fields[0].borrow().expect_int()?;
            let height = fields[1].borrow().expect_int()?;

            Ok(UVec2::new(width as u16, height as u16))
        }
        _ => Err(ValueError::TypeError("not a uvec2".to_string())),
    }
}

pub fn register_print<C>(modules: &Modules, externals: &mut ExternalFunctions<C>) {
    let mangrove_std_symbol_table = &modules
        .get(&["mangrove".to_string(), "std".to_string()])
        .unwrap()
        .symbol_table;

    register_print_internal(&mangrove_std_symbol_table, externals);
}

fn register_print_internal<C>(std_module: &SymbolTable, externals: &mut ExternalFunctions<C>) {
    let print_id = std_module
        .get_external_function_declaration("print")
        .unwrap()
        .id;

    externals
        .register_external_function(print_id, move |args: &[VariableValue], _context| {
            if let Some(value) = args.first() {
                let display_value = value.convert_to_string_if_needed();
                println!("{display_value}");
                Ok(Value::Unit)
            } else {
                Err(ValueError::WrongNumberOfArguments {
                    expected: 1,
                    got: 0,
                })?
            }
        })
        .expect("should work to register");
}

#[derive(Debug)]
pub struct DecoratedParseErr {
    pub span: Span,
    pub specific: SpecificError,
}
use chrono::{DateTime, Utc};

pub fn compile(
    module_path: &[String],
    source_map: &mut SourceMap,
) -> Result<Program, MangroveError> {
    let start = Instant::now();

    //let mut root_versions = SeqMap::new();
    //root_versions.insert("mangrove".to_string(), "0.0.0".parse().unwrap())?;

    let program = compile_and_analyze(module_path, source_map)?;

    let end = Instant::now();
    let duration = end.duration_since(start);

    let now: DateTime<Utc> = Utc::now();
    eprintln!(
        "{} {}: {} {} {:?}",
        now.format("%Y-%m-%d %H:%M:%S").white(),
        "compiled".bright_cyan(),
        module_path.join("::").green(),
        "took".bright_cyan(),
        duration.blue(),
    );

    Ok(program)
}

/*
pub fn compile_internal<C>(
    module_path: &[String],
    resolved_program: &mut Program,
    externals: &mut ExternalFunctions<C>,
    source_map: &mut SourceMap,
) -> Result<ModuleRef, MangroveError> {
    let relative_path = module_path_to_relative_swamp_file_string(module_path);
    let parsed_module = parse_single_module(&relative_path, source_map)?;

    let main_module = prepare_main_module(&mut resolved_program.state, externals, module_path)?;

    let main_path = module_path;

    let main_module_ref = Rc::new(main_module);
    resolved_program.modules.add(main_module_ref.clone());

    resolved_program.modules.add(Rc::new(create_std_module()));

    let mut dependency_parser = DependencyParser::new();
    dependency_parser.add_ast_module(Vec::from(main_path), parsed_module);

    for module_path in resolved_program.modules.modules.keys() {
        dependency_parser.add_resolved_module(module_path.clone());
    }

    Ok(main_module_ref)
}


 */
