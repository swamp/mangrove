use swamp_script::prelude::IdentifierName;
use swamp_script_semantic::{ResolvedInternalFunctionDefinitionRef, ResolvedStructTypeRef};

pub fn get_impl_func(
    struct_type_ref: &ResolvedStructTypeRef,
    name: &str,
) -> ResolvedInternalFunctionDefinitionRef {
    let identifier_name = IdentifierName(name.to_string());
    struct_type_ref
        .borrow()
        .get_internal_member(identifier_name)
        .expect(&format!("must have function {}", name))
}

pub fn get_impl_func_optional(
    struct_type_ref: &ResolvedStructTypeRef,
    name: &str,
) -> Option<ResolvedInternalFunctionDefinitionRef> {
    let identifier_name = IdentifierName(name.to_string());

    struct_type_ref
        .borrow()
        .get_internal_member(identifier_name)
}
