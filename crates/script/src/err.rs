/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::script::{DecoratedParseErr, MangroveError};
use crate::ErrorResource;
use swamp::prelude::{App, Plugin};
use swamp_script::prelude::{show_error, show_execute_error, show_parse_error, SourceMap};

pub fn show_mangrove_error(err: &MangroveError, source_map: &SourceMap) {
    match err {
        MangroveError::IoError(_) => todo!(),
        MangroveError::DecoratedParseError(decorated_parse_error) => {
            show_decorated(decorated_parse_error, source_map);
        }
        MangroveError::ExecuteError(err) => show_execute_error(err, source_map),
        MangroveError::Other(_) => todo!(),
        MangroveError::ScriptResolveError(_) => todo!(),
        MangroveError::SemanticError(_) => todo!(),
        MangroveError::ResolveError(resolve_err) => show_error(resolve_err, source_map),
        MangroveError::DepLoaderError(_) => todo!(),
        MangroveError::SeqMapError(_) => todo!(),
    }
}

pub fn show_decorated(err: &DecoratedParseErr, source_map: &SourceMap) {
    show_parse_error(&err.specific, &err.span, source_map);
}

pub struct ErrorPlugin;

impl Plugin for ErrorPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ErrorResource { has_errors: false });
    }
}
