use swamp_script::prelude::*;

pub fn get_impl_func(
    struct_type_ref: &ResolvedStructTypeRef,
    name: &str,
) -> ResolvedInternalFunctionDefinitionRef {
    struct_type_ref
        .borrow()
        .get_internal_member_function(name)
        .expect(&format!("must have function {}", name))
        .clone()
}

pub fn get_impl_func_optional(
    struct_type_ref: &ResolvedStructTypeRef,
    name: &str,
) -> Option<ResolvedInternalFunctionDefinitionRef> {
    struct_type_ref.borrow().get_internal_member_function(name)
}
