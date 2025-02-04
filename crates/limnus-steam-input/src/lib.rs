use limnus_app::prelude::{App, Plugin};
use limnus_default_stages::Update;
use limnus_local_resource::prelude::LocalResource;
use limnus_resource::prelude::Resource;
use limnus_steamworks::SteamworksClient;
use limnus_system_params::{LoRe, Re};
use seq_map::SeqMap;
use std::fmt::{Debug, Formatter};
use steamworks::{ClientManager, Input};
use tracing::info;

// https://partner.steamgames.com/doc/api/isteaminput

pub struct SteamworksGamepad {}

#[derive(LocalResource)]
pub struct SteamworksInput {
    pub manager: Input<ClientManager>,
    pub gamepads: SeqMap<u64, SteamworksGamepad>,
}

impl Debug for SteamworksInput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "steamworks input")
    }
}

pub fn debug_tick(input: LoRe<SteamworksInput>, bindings: Re<SteamworksInputBindings>) {
    input.manager.run_frame();
    let controllers = input.manager.get_connected_controllers();
    for controller_id in controllers {
        info!(?controller_id, "active controller");
        for (_action_set_name, bindings) in &bindings.action_sets.sets {
            // TODO: Find out how to get action sets to work
            /*
            input
                .manager
                .activate_action_set_handle(controller_id, bindings.handle);

             */

            for analog in &bindings.analog {
                let data = input
                    .manager
                    .get_analog_action_data(controller_id, analog.handle);

                // TODO: eMode: EInputSourceMode
                let x = data.x; // needed because it is packed
                let y = data.y; // needed because it is packed
                let active = data.bActive; // needed because it is packed
                info!(x=?x,y=?y, active=active, name=analog.debug_name, "analog data");
            }

            for digital in &bindings.digital {
                let data = input
                    .manager
                    .get_digital_action_data(controller_id, digital.handle);
                let value = data.bState; // needed because it is packed
                let active = data.bActive; // needed because it is packed
                if value {
                    info!(?value, ?active, name = digital.debug_name, "digital data");
                }
            }
        }
    }
}

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
pub struct SteamworksInputConfig {
    pub action_sets: ActionSets,
}

#[derive(Debug)]
pub struct DigitalActionBinding {
    pub debug_name: String,
    pub handle: u64,
}

#[derive(Debug)]
pub struct AnalogActionBinding {
    pub debug_name: String,
    pub handle: u64,
}

#[derive(Debug)]
pub struct ActionBindings {
    pub handle: u64,
    pub digital: Vec<DigitalActionBinding>,
    pub analog: Vec<AnalogActionBinding>,
}

#[derive(Debug)]
pub struct ActionBindingSets {
    pub sets: SeqMap<String, ActionBindings>,
}

#[derive(Debug, Resource)]
pub struct SteamworksInputBindings {
    pub action_sets: ActionBindingSets,
}

pub struct SteamworksInputPlugin;

impl Plugin for SteamworksInputPlugin {
    fn build(&self, app: &mut App) {
        info!("booting up steam input");

        let client = app.get_resource_mut::<SteamworksClient>().unwrap();
        let input = client.client.input();

        let config = app.get_resource_ref::<SteamworksInputConfig>().unwrap();

        let mut bindings = SteamworksInputBindings {
            action_sets: ActionBindingSets {
                sets: SeqMap::default(),
            },
        };

        for (set_name, actions_in_set) in &config.action_sets.sets {
            // TODO: Find out how to get action sets to work
            //let handle = input.get_action_set_handle(&*set_name);
            //assert_ne!(handle, 0, "wrong action set handle {}", set_name);

            let mut binding_set = ActionBindings {
                handle: 0,
                digital: vec![],
                analog: vec![],
            };

            for action in &actions_in_set.analog {
                info!(name = action.name, "binding analog");
                let handle = input.get_analog_action_handle(&action.name);
                //assert_ne!(handle, 0, "wrong analog action handle {}", action.name);
                binding_set.analog.push(AnalogActionBinding {
                    debug_name: action.name.clone(),
                    handle,
                });
            }

            for digital in &actions_in_set.digital {
                info!(name = digital.name, "binding digital");
                let handle = input.get_digital_action_handle(&digital.name);
                //assert_ne!(handle, 0, "wrong digital action handle {}", digital.name);
                binding_set.digital.push(DigitalActionBinding {
                    debug_name: digital.name.clone(),
                    handle,
                });
            }

            bindings
                .action_sets
                .sets
                .insert(set_name.clone(), binding_set)
                .unwrap();
        }

        input.init(true);

        app.insert_local_resource(SteamworksInput {
            manager: input,
            gamepads: SeqMap::default(),
        });

        app.insert_resource(bindings);
        app.add_system(Update, debug_tick);
    }
}
