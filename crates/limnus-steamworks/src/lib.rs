use limnus_app::prelude::{App, Plugin};
use limnus_default_stages::PreUpdate;
use limnus_local_resource::prelude::LocalResource;
use limnus_resource::prelude::Resource;
use limnus_system_params::LoReM;
use std::fmt::{Debug, Formatter};
use steamworks::{Client, SingleClient};

#[derive(Resource)]
pub struct SteamworksClient {
    pub client: Client,
}

impl Debug for SteamworksClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "steamworksclient (multithreaded)")
    }
}

#[derive(LocalResource)]
pub struct SteamworksClientSingleThread {
    pub single: SingleClient,
}

impl Debug for SteamworksClientSingleThread {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "steamworksclient (single)")
    }
}

pub fn run_callbacks(mut single_threaded: LoReM<SteamworksClientSingleThread>) {
    single_threaded.single.run_callbacks();
}

pub struct SteamworksPlugin;

impl Plugin for SteamworksPlugin {
    fn build(&self, app: &mut App) {
        let (client, single) = Client::init().unwrap();

        app.add_system(PreUpdate, run_callbacks);

        app.insert_resource(SteamworksClient { client });
        app.insert_local_resource(SteamworksClientSingleThread { single });
    }
}
