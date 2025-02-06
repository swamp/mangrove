use limnus_app::prelude::{App, Plugin};
use limnus_default_stages::{PreUpdate, Update};
use limnus_input::Controllers;
use limnus_input_binding::InputConfig;
use limnus_local_resource::prelude::LocalResource;
use limnus_resource::prelude::Resource;
use limnus_steamworks::SteamworksClient;
use limnus_system_params::{LoRe, LoReM, Re, ReAll, ReM};
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
    is_initialized: bool,
}

impl Debug for SteamworksInput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "steamworks input")
    }
}

pub fn waiting_for_flaky_steam_input_to_load(
    input_config: Re<InputConfig>,
    mut input: LoReM<SteamworksInput>,
    mut re_all: ReAll,
) {
    if input.is_initialized {
        return;
    }

    let first = input_config.action_sets.sets.iter().next();
    if let Some(found_first) = first {
        let name = found_first.0;
        info!(name, "testing");
        let handle = input.manager.get_action_set_handle(name);
        if handle == 0 {
            return;
        }

        info!("Steam INPUT HAS FINALLY LOADED A CONFIGURATION! trying to bind everything");

        let bindings = convert_bindings(&input_config, &input.manager);

        re_all.insert(bindings);

        input.is_initialized = true;
    }
}

/// # Panics
/// Must have at least one action set stored
pub fn get_action_set_for_controller(
    _device_id: u64,
    bindings: &SteamworksInputBindings,
) -> &ActionBindings {
    let (name, bindings) = bindings.action_sets.sets.iter().next().unwrap();
    info!(name, "selecting set");
    bindings
}

pub fn debug_tick(
    input: LoRe<SteamworksInput>,
    bindings: Re<SteamworksInputBindings>,
    mut controllers: ReM<Controllers>,
) {
    input.manager.run_frame();
    let connected_controllers = input.manager.get_connected_controllers();
    for controller_id in &connected_controllers {
        //        info!(?controller_id, "active controller");
        let bindings = get_action_set_for_controller(*controller_id, &bindings);
    }

    for controller_id in &connected_controllers {
        //        info!(?controller_id, "active controller");
        let bindings = get_action_set_for_controller(*controller_id, &bindings);

        // TODO: DO not set action set every frame
        input
            .manager
            .activate_action_set_handle(*controller_id, bindings.handle);

        for analog in &bindings.analog {
            let data = input
                .manager
                .get_analog_action_data(*controller_id, analog.handle);

            // TODO: eMode: EInputSourceMode
            let x = data.x; // needed because it is packed
            let y = data.y; // needed because it is packed
            let active = data.bActive; // needed because it is packed

            if x.abs() > 0.1 || y.abs() > 0.1 {
                info!(x=?x,y=?y, active=active, name=analog.debug_name, "analog data");
            }
        }

        for digital in &bindings.digital {
            let data = input
                .manager
                .get_digital_action_data(*controller_id, digital.handle);
            let value = data.bState; // needed because it is packed
            let active = data.bActive; // needed because it is packed
            if value {
                info!(?value, ?active, name = digital.debug_name, "digital data");
            }
        }
    }
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

fn convert_bindings(config: &InputConfig, input: &Input<ClientManager>) -> SteamworksInputBindings {
    let mut bindings = SteamworksInputBindings {
        action_sets: ActionBindingSets {
            sets: SeqMap::default(),
        },
    };

    for (set_name, actions_in_set) in &config.action_sets.sets {
        // TODO: Find out how to get action sets to work
        let converted_set_name = set_name.clone();
        let handle = input.get_action_set_handle(&converted_set_name);
        info!(handle, converted_set_name, "set handle");
        //assert_ne!(handle, 0, "wrong action set handle {}", set_name);

        let mut binding_set = ActionBindings {
            handle,
            digital: vec![],
            analog: vec![],
        };

        for analog in &actions_in_set.analog {
            let converted_name = analog.name.clone();
            info!(converted_name, "binding analog");
            let handle = input.get_analog_action_handle(&converted_name);
            info!(handle, "analog handle");
            //assert_ne!(handle, 0, "wrong analog action handle {}", action.name);
            binding_set.analog.push(AnalogActionBinding {
                debug_name: converted_name,
                handle,
            });
        }

        for digital in &actions_in_set.digital {
            let converted_name = digital.name.clone();
            info!(converted_name, "binding digital");
            let handle = input.get_digital_action_handle(&converted_name);
            info!(handle, "digital handle");
            //assert_ne!(handle, 0, "wrong digital action handle {}", digital.name);
            binding_set.digital.push(DigitalActionBinding {
                debug_name: converted_name,
                handle,
            });
        }

        bindings
            .action_sets
            .sets
            .insert(converted_set_name, binding_set)
            .unwrap();
    }

    bindings
}

impl Plugin for SteamworksInputPlugin {
    fn build(&self, app: &mut App) {
        info!("booting up steam input");

        let client = app.get_resource_mut::<SteamworksClient>().unwrap();

        let input = client.client.input();
        input.init(false);
        app.insert_local_resource(SteamworksInput {
            manager: input,
            gamepads: SeqMap::default(),
            is_initialized: false,
        });

        info!("steam input is initialized");

        app.add_system(PreUpdate, waiting_for_flaky_steam_input_to_load);
        app.add_system(Update, debug_tick);
    }
}
