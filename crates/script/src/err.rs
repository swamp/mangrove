/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::ErrorResource;
use crate::script::{DecoratedParseErr, MangroveError};
use std::env::current_dir;
use std::path::Path;
use swamp::prelude::{App, Plugin};
use swamp_script::prelude::{
    DepLoaderError, SourceMap, show_analyzer_error, show_dependency_error, show_parse_error,
    show_runtime_error, show_script_resolve_error, show_semantic_error,
};
use tracing::error;

pub fn show_mangrove_error(err: &MangroveError, source_map: &SourceMap) {
    let current_path = &*current_dir().unwrap();
    match err {
        MangroveError::DecoratedParseError(decorated_parse_error) => {
            show_parse_error(
                &decorated_parse_error.specific,
                &decorated_parse_error.span,
                source_map,
                current_path,
            );
        }
        MangroveError::ExecuteError(err) => show_runtime_error(err, source_map, current_path),
        MangroveError::Other(description) => error!(?err, ?description, "unknown error"),
        MangroveError::SemanticError(err) => show_semantic_error(err, source_map, current_path),
        MangroveError::Error(resolve_err) => {
            show_analyzer_error(resolve_err, source_map, current_path)
        }
        MangroveError::DepLoaderError(err) => match err {
            DepLoaderError::DependencyError(err) => {
                show_dependency_error(err, source_map, current_path)
            }
        },
        MangroveError::ScriptError(_) => todo!(),
        MangroveError::EvalLoaderError(err) => todo!(), //show_eval_loader_error(err, source_map),
        _ => {}
    }
}

pub struct ErrorPlugin;

impl Plugin for ErrorPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ErrorResource { has_errors: false });
    }
}
