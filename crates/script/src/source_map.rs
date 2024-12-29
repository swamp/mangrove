use crate::SourceMapResource;
use swamp::prelude::{App, Plugin};
use swamp_script::prelude::{ResolvedNode, SourceMap, SourceMapLookup, Span};

pub struct SourceMapPlugin;

#[derive(Debug)]
pub struct SourceMapWrapper {
    pub source_map: SourceMap,
}

impl SourceMapLookup for SourceMapWrapper {
    fn get_text(&self, resolved_node: &ResolvedNode) -> &str {
        self.source_map.get_span_source(
            resolved_node.span.file_id,
            resolved_node.span.offset as usize,
            resolved_node.span.length as usize,
        )
    }

    fn get_text_span(&self, span: &Span) -> &str {
        self.source_map
            .get_span_source(span.file_id, span.offset as usize, span.length as usize)
    }
}

impl Plugin for SourceMapPlugin {
    fn build(&self, app: &mut App) {
        //app.add_system(UpdatePhase::Update, detect_reload_tick);
        app.insert_resource(SourceMapResource {
            wrapper: SourceMapWrapper {
                source_map: SourceMap::new("scripts/".as_ref()),
            },
        });
    }
}
