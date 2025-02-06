use limnus_resource::prelude::Resource;
use seq_map::SeqMap;

#[derive(Debug)]
pub struct DigitalAction {
    pub name: String,
}

#[derive(Debug)]
pub struct AnalogAction {
    pub name: String,
}

#[derive(Debug)]
pub struct Actions {
    pub digital: Vec<DigitalAction>,
    pub analog: Vec<AnalogAction>,
}

#[derive(Debug)]
pub struct ActionSets {
    pub sets: SeqMap<String, Actions>,
}

#[derive(Debug, Resource)]
pub struct InputConfig {
    pub action_sets: ActionSets,
}
