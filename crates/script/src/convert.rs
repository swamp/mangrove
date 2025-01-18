use crate::err::show_mangrove_error;
use crate::logic::{boot, ScriptLogic};
use crate::{ErrorResource, ScriptMessage, SourceMapResource};
use limnus_gamepad::{Button, GamepadMessage};
use seq_map::SeqMap;
use std::cell::RefCell;
use std::rc::Rc;
use swamp::prelude::{App, FixedPostUpdate, LoRe, LoReM, LocalResource, Msg, Plugin, ReM};
use swamp_script::prelude::{
    overwrite_struct, overwrite_value, quick_deserialize, ResolvedAnonymousStructType,
    ResolvedNode, ResolvedStructType, ResolvedStructTypeRef, ResolvedType, Value, ValueRef,
};
use tracing::info;

pub fn detect_reload_tick(
    script_messages: Msg<ScriptMessage>,
    mut script_logic: LoReM<ScriptLogic>,
    mut previous_logic: LoReM<PreviousLogic>,
) {
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => {
                info!(len=%previous_logic.payload.len(), "reload detected");
                if let Some(previous_struct_type_ref) = &previous_logic.type_ref {
                    let (deserialized_value, deserialized_octet_size) = quick_deserialize(
                        &ResolvedType::Struct(previous_struct_type_ref.clone()),
                        &previous_logic.payload,
                        0,
                    );
                    previous_logic.type_ref = Some(previous_struct_type_ref.clone());
                    info!(%deserialized_value, %deserialized_octet_size, "deserialized value");
                    let overwritten_value = {
                        let overwrite_ref = script_logic.mutable_logic_value_ref();
                        overwrite_value(overwrite_ref, deserialized_value);
                        overwrite_ref.borrow().clone()
                    };
                    info!(%overwritten_value, "overwritten value");
                    script_logic.debug_set_logic_value(overwritten_value);
                } else {
                    match script_logic.immutable_logic_value() {
                        Value::Struct(first_time_struct_ref, _) => {
                            previous_logic.type_ref = Some(first_time_struct_ref.clone());
                        }
                        _ => panic!("must be struct"),
                    }
                }
            }
        }
    }
}

pub fn store_tick(script_logic: LoRe<ScriptLogic>, mut previous_logic: LoReM<PreviousLogic>) {
    let mut buf = [0u8; 2048];
    let size = script_logic
        .immutable_logic_value()
        .quick_serialize(&mut buf, 0);

    previous_logic.payload = buf[..size].to_vec();
    //info!(%size, "store payload");
}

#[derive(Debug, LocalResource)]
pub struct PreviousLogic {
    type_ref: Option<ResolvedStructTypeRef>,
    payload: Vec<u8>,
}

pub struct ConvertPlugin;

impl Plugin for ConvertPlugin {
    fn build(&self, app: &mut App) {
        app.insert_local_resource(PreviousLogic {
            type_ref: None,
            payload: vec![],
        });
        app.add_system(FixedPostUpdate, detect_reload_tick);
        app.add_system(FixedPostUpdate, store_tick);
    }
}
