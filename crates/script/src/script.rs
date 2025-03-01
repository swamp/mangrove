/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::render::MathTypes;
use seq_map::SeqMapError;
use std::cell::RefCell;
use std::fmt::{Debug, Display, Formatter};
use std::io;
use std::rc::Rc;
use swamp::prelude::{Color, Rotation, SpriteParams, UVec2, Vec2, Vec3};
use swamp_script::compile_and_analyze;
use swamp_script::prelude::*;

#[derive(Debug)]
pub enum MangroveError {
    IoError(io::Error),
    DecoratedParseError(DecoratedParseErr),
    ExecuteError(ExecuteError),
    Other(String),
    ScriptError(ScriptError),
    SemanticError(SemanticError),
    Error(Error),
    DepLoaderError(DepLoaderError),
    SeqMapError(SeqMapError),
    EvalLoaderError(EvalLoaderError),
}

impl Display for MangroveError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<io::Error> for MangroveError {
    fn from(value: io::Error) -> Self {
        Self::IoError(value)
    }
}

impl From<ScriptError> for MangroveError {
    fn from(value: ScriptError) -> Self {
        Self::ScriptError(value)
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

impl From<Error> for MangroveError {
    fn from(value: Error) -> Self {
        Self::Error(value)
    }
}

impl From<DepLoaderError> for MangroveError {
    fn from(value: DepLoaderError) -> Self {
        Self::DepLoaderError(value)
    }
}

impl From<EvalLoaderError> for MangroveError {
    fn from(value: EvalLoaderError) -> Self {
        Self::EvalLoaderError(value)
    }
}

impl From<DecoratedParseErr> for MangroveError {
    fn from(value: DecoratedParseErr) -> Self {
        Self::DecoratedParseError(value)
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
) -> Result<StructTypeRef, Error> {
    Ok(symbol_table.add_generated_struct(name, &[("hidden", Type::Unit)])?)
}

pub fn create_empty_struct_value(struct_type: StructTypeRef) -> Value {
    Value::Struct(struct_type, [].to_vec())
}

pub fn create_empty_struct_value_util(
    symbol_table: &mut SymbolTable,
    name: &str,
) -> Result<(Value, StructTypeRef), Error> {
    let struct_type = create_empty_struct_type(symbol_table, name)?;
    Ok((create_empty_struct_value(struct_type.clone()), struct_type))
}

pub fn sprite_params(sprite_params_struct: &Value) -> Result<SpriteParams, ValueError> {
    if let Value::Struct(_struct_type_ref, fields) = sprite_params_struct {
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

pub fn create_default_color_value(color_struct_type_ref: StructTypeRef) -> Value {
    let fields = vec![
        Value::Float(Fp::one()), // red
        Value::Float(Fp::one()), // green
        Value::Float(Fp::one()), // blue
        Value::Float(Fp::one()), // alpha
    ];

    Value::Struct(color_struct_type_ref, value_to_value_ref(&fields))
}

pub fn value_to_value_ref(fields: &[Value]) -> Vec<ValueRef> {
    fields
        .iter()
        .map(|v| Rc::new(RefCell::new(v.clone())))
        .clone()
        .collect()
}

pub fn create_default_sprite_params(
    sprite_params_struct_type_ref: StructTypeRef,
    color_type: &StructTypeRef,
    math_types: &MathTypes,
) -> Value {
    let fields = vec![
        Value::Bool(false), // flip_x
        Value::Bool(false), // flip_y
        Value::Int(0),      // rotation
        create_default_color_value(color_type.clone()),
        Value::Int(1), // scale
        Value::Tuple(
            // texture_position (uv)
            math_types.pos2_tuple_type.clone(),
            value_to_value_ref(&[Value::Int(0), Value::Int(0)]),
        ),
        Value::Tuple(
            // texture_size
            math_types.size2_tuple_type.clone(),
            value_to_value_ref(&[Value::Int(0), Value::Int(0)]),
        ),
    ];

    Value::Struct(sprite_params_struct_type_ref, value_to_value_ref(&fields))
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
        Value::Struct(_, fields) => {
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

fn prepare_main_module<C>(
    state: &mut ProgramState,
    externals: &mut ExternalFunctions<C>,
    root_module_path: &[String],
) -> Result<Module, Error> {
    let any_parameter = TypeForParameter {
        name: String::default(),
        resolved_type: Type::String,
        is_mutable: false,
        node: None,
    };

    let print_id = state.allocate_external_function_id();

    let print_external = ExternalFunctionDefinition {
        name: None,
        assigned_name: "print".to_string(),
        signature: Signature {
            parameters: [any_parameter].to_vec(),
            return_type: Box::from(Type::Unit),
        },
        id: print_id,
    };

    let mut symbol_table = SymbolTable::default();

    symbol_table.add_external_function_declaration(print_external)?;
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

    let main_module = Module::new(root_module_path, symbol_table, None);

    Ok(main_module)
}

#[derive(Debug)]
pub struct DecoratedParseErr {
    pub span: Span,
    pub specific: SpecificError,
}

pub fn compile<C>(
    module_path: &[String],
    analyzed_program: &mut Program,
    externals: &mut ExternalFunctions<C>,
    source_map: &mut SourceMap,
) -> Result<ModuleRef, MangroveError> {
    let std_module =
        prepare_main_module(&mut analyzed_program.state, externals, &["std".to_string()])?;

    analyzed_program
        .auto_use_modules
        .modules
        .push(std_module.namespace.symbol_table);

    let core_module = create_std_module();
    analyzed_program
        .auto_use_modules
        .modules
        .push(core_module.namespace.symbol_table);

    compile_and_analyze(module_path, analyzed_program, source_map)?;
    Ok(analyzed_program.modules.get(module_path).unwrap().clone())
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
