use crate::script::{DecoratedParseErr, MangroveError};
use swamp_script::prelude::{SourceMap, Span};

pub fn show_parse_error(err: &DecoratedParseErr, source_map: &SourceMap) {
    let error_text = format!("{:?}", err.specific);
    show_error(&err.span, &*error_text, &source_map);
}

pub fn show_error(span: &Span, error_text: &str, source_map: &SourceMap) {
    let source =
        source_map.get_span_source(span.file_id, span.offset as usize, span.length as usize);
    let (col, row) = source_map.get_span_location_utf8(span.file_id, span.offset as usize);
    let file_info = source_map.fetch_relative_filename(span.file_id);

    eprintln!("err: {file_info}: {row}:{col} {error_text}:\n{source}");
}

pub fn show_mangrove_error(err: MangroveError, source_map: &SourceMap) {
    match err {
        MangroveError::IoError(_) => todo!(),
        MangroveError::DecoratedParseError(decorated_parse_error) => {
            show_parse_error(&decorated_parse_error, source_map)
        }
        MangroveError::ExecuteError(_) => todo!(),
        MangroveError::Other(_) => todo!(),
        MangroveError::ScriptResolveError(_) => todo!(),
        MangroveError::SemanticError(_) => todo!(),
        MangroveError::ResolveError(_) => todo!(),
        MangroveError::DepLoaderError(_) => todo!(),
        MangroveError::SeqMapError(_) => todo!(),
    }
}
