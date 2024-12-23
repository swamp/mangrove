use crate::script::{DecoratedParseErr, MangroveError};
use swamp_script::prelude::{ResolveError, SourceMap, Span, Spanned};


pub fn show_parse_error(err: &DecoratedParseErr, source_map: &SourceMap) {
    let error_text = format!("{:?}", err.specific);
    show_error(&err.span, &*error_text, &source_map);
}

pub fn show_error(span: &Span, error_text: &str, source_map: &SourceMap) {
    let source =
        source_map.get_span_source(span.file_id, span.offset as usize, span.length as usize);
    let (row, col) = source_map.get_span_location_utf8(span.file_id, span.offset as usize);
    let file_info = source_map.fetch_relative_filename(span.file_id);

    eprintln!("err: {file_info}: {row}:{col} {error_text}:\n{source}");
}

pub fn show_resolve_error(err: ResolveError, source_map: &SourceMap) {
    match err {
        ResolveError::NamespaceError(_) => todo!(),
        ResolveError::CanNotFindModule(_) => todo!(),
        ResolveError::UnknownStructTypeReference(_) => todo!(),
        ResolveError::UnknownLocalStructTypeReference(_) => todo!(),
        ResolveError::DuplicateFieldName(_) => todo!(),
        ResolveError::Unknown(_) => todo!(),
        ResolveError::UnknownImplTargetTypeReference(_) => todo!(),
        ResolveError::WrongFieldCountInStructInstantiation(_, _) => todo!(),
        ResolveError::MissingFieldInStructInstantiation(_, _) => todo!(),
        ResolveError::ExpectedFunctionExpression => todo!(),
        ResolveError::CouldNotFindMember(_, _) => todo!(),
        ResolveError::UnknownVariable(_) => todo!(),
        ResolveError::NotAnArray(_) => todo!(),
        ResolveError::ArrayIndexMustBeInt(_) => todo!(),
        ResolveError::OverwriteVariableWithAnotherType(_) => todo!(),
        ResolveError::WrongNumberOfArguments(_, _) => todo!(),
        ResolveError::IncompatibleArguments(_, _) => todo!(),
        ResolveError::CanOnlyOverwriteVariableWithMut(_) => todo!(),
        ResolveError::OverwriteVariableNotAllowedHere(_) => todo!(),
        ResolveError::NotNamedStruct(_) => todo!(),
        ResolveError::UnknownEnumVariantType(_) => todo!(),
        ResolveError::WasNotStructType(_) => todo!(),
        ResolveError::UnknownStructField(_) => todo!(),
        ResolveError::MustBeEnumType(_) => todo!(),
        ResolveError::UnknownEnumVariantTypeInPattern(_) => todo!(),
        ResolveError::ExpectedEnumInPattern(_) => todo!(),
        ResolveError::WrongEnumVariantContainer(_) => todo!(),
        ResolveError::VariableIsNotMutable(_) => todo!(),
        ResolveError::ArgumentIsNotMutable => todo!(),
        ResolveError::WrongNumberOfTupleDeconstructVariables => todo!(),
        ResolveError::UnknownTypeReference(x) => {
            show_error(&x.span, "Unknown type reference", &source_map)
        }
        ResolveError::SemanticError(_) => todo!(),
        ResolveError::SeqMapError(_) => todo!(),
        ResolveError::ExpectedMemberCall(_) => todo!(),
        ResolveError::CouldNotFindStaticMember(_, _) => todo!(),
        ResolveError::TypeAliasNotAStruct(_) => todo!(),
        ResolveError::ModuleNotUnique => todo!(),
        ResolveError::ExpressionIsOfWrongFieldType => todo!(),
        ResolveError::ExpectedOptional => todo!(),
        ResolveError::ExpectedVariable => todo!(),
        ResolveError::EmptyMapLiteral => todo!(),
        ResolveError::MapKeyTypeMismatch { .. } => todo!(),
        ResolveError::MapValueTypeMismatch { .. } => todo!(),
        ResolveError::TypeIsNotAnIndexCollection(_) => todo!(),
        ResolveError::NotSameKeyTypeForMapIndex(_, _) => todo!(),
        ResolveError::NonUniqueKeyValueInMap(_) => todo!(),
        ResolveError::UnknownIndexAwareCollection => todo!(),
        ResolveError::InvalidOperatorForArray(_) => todo!(),
        ResolveError::IncompatibleTypes(_, _) => todo!(),
        ResolveError::ExpectedArray(_) => todo!(),
        ResolveError::UnknownMemberFunction(_) => todo!(),
        ResolveError::WrongNumberOfTypeArguments(_, _) => todo!(),
        ResolveError::OnlyVariablesAllowedInEnumPattern => todo!(),
        ResolveError::ExpressionsNotAllowedInLetPattern => todo!(),
        ResolveError::UnknownField(_) => todo!(),
        ResolveError::EnumVariantHasNoFields(_) => todo!(),
        ResolveError::TooManyTupleFields { .. } => todo!(),
        ResolveError::NotInFunction => todo!(),
        ResolveError::ExpectedBooleanExpression => todo!(),
        ResolveError::NotAnIterator(resolved_type) => {
            show_error(&resolved_type.span(), "Not an iterator", &source_map)
        }
        ResolveError::UnsupportedIteratorPairs => todo!(),
        ResolveError::NeedStructForFieldLookup => todo!(),
        ResolveError::IntConversionError(_) => todo!(),
        ResolveError::FloatConversionError(_) => todo!(),
        ResolveError::BoolConversionError => todo!(),
        ResolveError::DuplicateFieldInStructInstantiation(_, _) => todo!(),
        ResolveError::InternalError(_) => todo!(),
    }
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
        MangroveError::ResolveError(resolve_err) => show_resolve_error(resolve_err, source_map),
        MangroveError::DepLoaderError(_) => todo!(),
        MangroveError::SeqMapError(_) => todo!(),
    }
}
