/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/mangrove
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */

mod script;

use crate::script::Script;
use limnus_message::prelude::Message;

use swamp::prelude::*;

pub fn logic_tick(mut script: LoReM<Script>) {
    script.tick().expect("script.tick() crashed");
}

pub fn render_tick(mut script: LoReM<Script>, mut wgpu_render: ReM<Render>) {
    script
        .render(&mut wgpu_render)
        .expect("script.render() crashed");
}

pub fn flush_render_tick(
    script: LoRe<Script>,
    wgpu_window: Re<WgpuWindow>,
    mut wgpu_render: ReM<Render>,
    materials: Re<LimnusAssets<Material>>,
    fonts: Re<LimnusAssets<Font>>,
) {
    let now = script.now();

    wgpu_window
        .render(wgpu_render.clear_color(), |render_pass| {
            wgpu_render.render(render_pass, &materials, &fonts, now)
        })
        .unwrap();
}

pub fn detect_reload_tick(script_messages: Msg<ScriptMessage>) {
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => info!("SHOULD RELOAD NOW!"),
        }
    }
}
#[derive(Message, Debug)]
pub enum ScriptMessage {
    Reload,
}

pub struct ScriptPlugin;

impl Plugin for ScriptPlugin {
    fn build(&self, app: &mut App) {
        let all_resources = app.resources_mut();
        let script = Script::new(all_resources).expect("script.boot() crashed");
        app.create_message_type::<ScriptMessage>();

        app.add_system(UpdatePhase::Update, detect_reload_tick);
        app.add_system(UpdatePhase::Update, logic_tick);
        app.add_system(UpdatePhase::Update, render_tick);
        app.add_system(UpdatePhase::Update, flush_render_tick);
        app.insert_local_resource(script);
    }
}
