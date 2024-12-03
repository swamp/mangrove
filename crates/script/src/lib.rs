mod script;

use crate::script::Script;

use swamp::prelude::*;

pub fn script_tick(
    mut script: LoReM<Script>,
    window: Re<WgpuWindow>,
    mut wgpu_render: ReM<Render>,
    mut all_resources: ReAll,
) {
    let now = script.now();

    script.tick();
}

pub struct ScriptPlugin;

impl Plugin for ScriptPlugin {
    fn build(&self, app: &mut App) {
        let mut all_resources = app.resources_mut();
        let mut script = Script::new();

        script.boot(&mut all_resources);

        app.add_system(UpdatePhase::Update, script_tick);
        app.insert_local_resource(script);
    }
}
