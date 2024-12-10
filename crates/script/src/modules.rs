use swamp::prelude::{App, LocalResource, Plugin};
use swamp_script_semantic::ResolvedModules;

#[derive(LocalResource, Debug)]
pub struct ScriptModules {
    pub modules: ResolvedModules,
}

pub struct ScriptModulesPlugin;

impl Plugin for ScriptModulesPlugin {
    fn build(&self, app: &mut App) {
        let script_modules = ScriptModules {
            modules: Default::default(),
        };
        app.insert_local_resource(script_modules);
    }
}
