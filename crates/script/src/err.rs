/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use crate::ErrorResource;
use crate::script::MangroveError;
use std::env::current_dir;
use swamp::prelude::{App, Plugin};
use swamp_script::prelude::{SourceMap, show_runtime_error, show_script_resolve_error};
use tracing::error;

/// # Panics
///
pub fn show_mangrove_error(err: &MangroveError, source_map: &SourceMap) {
    let current_path = &*current_dir().unwrap();
    match err {
        MangroveError::ScriptResolveError(script_resolve_error) => {
            //show_script_resolve_error(script_resolve_error, source_map, current_path);
            // Should already be handled
        }
        MangroveError::RuntimeError(err) => show_runtime_error(err, source_map, current_path),
        MangroveError::Other(description) => error!(?err, ?description, "unknown error"),
    }
}

pub struct ErrorPlugin;

impl Plugin for ErrorPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ErrorResource { has_errors: false });
    }
}
