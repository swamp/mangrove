/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use std::cell::RefCell;
use std::rc::Rc;
use swamp_script::prelude::*;

pub fn get_impl_func(
    associated_impls: &AssociatedImpls,
    struct_type_ref: &NamedStructType,
    name: &str,
) -> InternalFunctionDefinitionRef {
    associated_impls
        .get_internal_member_function(&Type::NamedStruct(struct_type_ref.clone()), name)
        .unwrap_or_else(|| panic!("must have function {name}"))
        .clone()
}

pub fn get_impl_func_optional(
    associated_impls: &AssociatedImpls,
    struct_type_ref: &NamedStructType,
    name: &str,
) -> Option<InternalFunctionDefinitionRef> {
    associated_impls
        .get_internal_member_function(&Type::NamedStruct(struct_type_ref.clone()), name)
        .cloned()
}

#[derive(Debug)]
pub struct ScriptModule<C> {
    self_state_value_ref: ValueRef,
    update_fn: InternalFunctionDefinitionRef,

    external_functions: ExternalFunctions<C>,
    constants: Constants,
    script_context: C,
    resolved_program: Program,
    main_module: ModuleRef,
}

impl<C> ScriptModule<C> {
    pub fn new(
        self_state_value_ref: ValueRef,
        update_fn: InternalFunctionDefinitionRef,
        external_functions: ExternalFunctions<C>,
        script_context: C,
        constants: Constants,
        resolved_program: Program,
        main_module: ModuleRef,
    ) -> Self {
        Self {
            self_state_value_ref,
            update_fn,
            external_functions,
            script_context,
            constants,
            resolved_program,
            main_module,
        }
    }

    pub fn main_module(&self) -> &ModuleRef {
        let root_module_path = &["input".to_string()].to_vec();

        self.resolved_program
            .modules
            .get(root_module_path)
            .expect("input module should exist in the resolved_program")
    }

    pub fn update(
        &mut self,
        arguments: &[Value],
        debug_source_map: Option<&dyn SourceMapLookup>,
    ) -> Result<Value, RuntimeError> {
        util_execute_member_function_mut(
            &self.external_functions,
            &self.constants,
            &self.update_fn,
            self.self_state_value_ref.clone(),
            arguments,
            &mut self.script_context,
            debug_source_map,
        )
    }
}

impl<C: Default> Default for ScriptModule<C> {
    fn default() -> Self {
        Self {
            self_state_value_ref: Rc::new(RefCell::new(Value::default())),
            update_fn: Rc::new(InternalFunctionDefinition {
                body: Expression {
                    ty: Type::Int,
                    node: Node::default(),
                    kind: ExpressionKind::Block(vec![]),
                },
                name: LocalIdentifier(Node::default()),
                assigned_name: "".to_string(),
                signature: Signature {
                    parameters: vec![],
                    return_type: Box::new(Type::Int),
                },
                variable_scopes: FunctionScopeState::new(Type::Unit),
                function_scope_state: Vec::default(),
                program_unique_id: 0,
            }),
            external_functions: ExternalFunctions::<C>::new(),
            constants: Constants::default(),
            script_context: Default::default(),
            resolved_program: Program::default(),
            main_module: Rc::new(Module {
                main_expression: None,
                symbol_table: SymbolTable::new(&[]),
            }),
        }
    }
}

/*
pub fn boot<C>(
    modules: Vec<&ModuleRef>,
    root_module_path: &[String],
    update_function_name: &str,
    mut script_context: C,
    source_map: &mut SourceMapResource,
) -> Result<ScriptModule<C>, MangroveError> {
    let mut resolved_program = Program::new();
    let mut external_functions = ExternalFunctions::<C>::new();

    compile(
        root_module_path,
        &mut resolved_program,
        &mut external_functions,
        &mut source_map.wrapper.source_map,
    )?;

    for module in modules {
        resolved_program.modules.add(module.clone());
    }

    let main_module = resolved_program
        .modules
        .get(root_module_path)
        .expect("could not find main module")
        .clone();

    let mut constants = Constants::new();
    eval_constants(
        &external_functions,
        &mut constants,
        &resolved_program.state,
        &mut script_context,
    )?;

    let self_state_value = {
        let main_expression = main_module
            .expression
            .as_ref()
            .expect("must have code within the input module");

        util_execute_expression(
            &external_functions,
            &constants,
            main_expression,
            &mut script_context,
            None,
        )?
    };

    let Value::NamedStruct(self_struct_type_ref, _) = &self_state_value else {
        return Err(MangroveError::Other("needs to be logic struct".to_string()));
    };

    let update_function = get_impl_func(
        &resolved_program.state.associated_impls,
        self_struct_type_ref,
        update_function_name,
    );

    // Convert it to a mutable (reference), so it can be mutated in update ticks
    let self_state_value_ref = Rc::new(RefCell::new(self_state_value));

    Ok(ScriptModule::new(
        self_state_value_ref,
        update_function,
        external_functions,
        script_context,
        constants,
        resolved_program,
        main_module.clone(),
    ))
}

 */
