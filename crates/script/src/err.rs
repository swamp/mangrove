/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */

use crate::script::MangroveError;
use swamp_script::prelude::{show_error, SourceMap};

pub fn show_mangrove_error(err: MangroveError, source_map: &SourceMap) {
    match err {
        MangroveError::IoError(_) => todo!(),
        MangroveError::DecoratedParseError(_decorated_parse_error) => {
            //  show_error(&decorated_parse_error, source_map)
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
