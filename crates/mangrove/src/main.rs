/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */
use limnus_steam_input::{
    ActionSets, Actions, AnalogAction, DigitalAction, SteamworksInputConfig, SteamworksInputPlugin,
};
#[cfg(feature = "steam")]
use limnus_steamworks::SteamworksPlugin;
use mangrove_save_detector::SaveDetectorPlugin;
use mangrove_script::convert::ConvertPlugin;
use mangrove_script::err::ErrorPlugin;
use mangrove_script::flow::ScriptFlowPlugin;
use mangrove_script::input::ScriptInputPlugin;
use mangrove_script::render::ScriptRenderPlugin;
use mangrove_script::serialize::SerializePlugin;
use mangrove_script::simulation::ScriptSimulationPlugin;
use mangrove_script::source_map::SourceMapPlugin;
use mangrove_script::ScriptPlugin;
use seq_map::SeqMap;
use swamp::prelude::SwampDefaultPlugins;
use swamp::prelude::*;

const OLD_SCHOOL_RESOLUTION_LOW: (u16, u16) = (320, 180);
#[allow(dead_code)]
const OLD_SCHOOL_RESOLUTION_HIGH: (u16, u16) = (
    OLD_SCHOOL_RESOLUTION_LOW.0 * 2,
    OLD_SCHOOL_RESOLUTION_LOW.1 * 2,
);
#[allow(dead_code)]
const INTERMEDIATE_RESOLUTION: (u16, u16) = (384, 216);

const RESOLUTION: (u16, u16) = OLD_SCHOOL_RESOLUTION_LOW;
const FACTOR: u16 = 5;

fn main() {
    let mut app = App::new();

    app.insert_resource(Window {
        title: "mangrove".to_string(),
        requested_surface_size: UVec2::new(RESOLUTION.0 * FACTOR, RESOLUTION.1 * FACTOR),
        minimal_surface_size: UVec2::new(RESOLUTION.0, RESOLUTION.1),
        mode: ScreenMode::WindowedOnTop,
    })
    .add_plugins(DefaultPlugins)
    .add_plugins(SwampDefaultPlugins)
    .add_plugins(SourceMapPlugin)
    .add_plugins(ErrorPlugin)
    .add_plugins(ScriptPlugin)
    .add_plugins(ScriptInputPlugin)
    .add_plugins(ScriptFlowPlugin)
    .add_plugins(ScriptSimulationPlugin)
    .add_plugins(ConvertPlugin)
    .add_plugins(SerializePlugin)
    .add_plugins(ScriptRenderPlugin)
    .add_plugins(SaveDetectorPlugin);

    #[cfg(feature = "steam")]
    {
        let in_game_controls_action_set = Actions {
            digital: vec![
                DigitalAction {
                    name: "Ability1".into(),
                },
                DigitalAction {
                    name: "Ability2".into(),
                },
            ],
            analog: vec![AnalogAction {
                name: "Move".to_string(),
            }],
        };

        let mut all_sets = SeqMap::new();

        all_sets
            .insert("In Game".into(), in_game_controls_action_set)
            .expect("could not insert action set");

        let steam_input_config = SteamworksInputConfig {
            action_sets: ActionSets { sets: all_sets },
        };

        app.insert_resource(steam_input_config);

        app.add_plugins(SteamworksPlugin);
        app.add_plugins(SteamworksInputPlugin);
    }

    app.run();
}
