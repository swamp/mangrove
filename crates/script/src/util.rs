/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use swamp_script::prelude::*;

pub fn get_impl_func(
    struct_type_ref: &ResolvedStructTypeRef,
    name: &str,
) -> ResolvedInternalFunctionDefinitionRef {
    struct_type_ref
        .borrow()
        .get_internal_member_function(name)
        .unwrap_or_else(|| panic!("must have function {name}"))
}

pub fn get_impl_func_optional(
    struct_type_ref: &ResolvedStructTypeRef,
    name: &str,
) -> Option<ResolvedInternalFunctionDefinitionRef> {
    struct_type_ref.borrow().get_internal_member_function(name)
}
