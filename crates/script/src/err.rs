/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */

use crate::script::{DecoratedParseErr, MangroveError};
use swamp_script::prelude::{show_error, show_parse_error, SourceMap};

pub fn show_mangrove_error(err: &MangroveError, source_map: &SourceMap) {
    match err {
        MangroveError::IoError(_) => todo!(),
        MangroveError::DecoratedParseError(decorated_parse_error) => {
            show_decorated_err(&decorated_parse_error, source_map)
        }
        MangroveError::ExecuteError(_) => todo!(),
        MangroveError::Other(_) => todo!(),
        MangroveError::ScriptResolveError(_) => todo!(),
        MangroveError::SemanticError(_) => todo!(),
        MangroveError::ResolveError(resolve_err) => show_error(&resolve_err, source_map),
        MangroveError::DepLoaderError(_) => todo!(),
        MangroveError::SeqMapError(_) => todo!(),
    }
}

pub fn show_decorated_err(err: &DecoratedParseErr, source_map: &SourceMap) {
    show_parse_error(&err.specific, &err.span, &source_map);
}

/*
pub fn show_error(err: &ResolveError, source_map: &SourceMap) {
    let builder = build_resolve_error(err);
    let report = builder.build();
    report.print(&source_map, stderr()).unwrap();
}

#[must_use]
pub fn build_resolve_error(err: &ResolveError) -> Builder<usize> {
 */
