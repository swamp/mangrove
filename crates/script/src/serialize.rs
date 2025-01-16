use crate::logic::ScriptLogic;
use swamp::prelude::{App, LoRe, LoReM, Plugin, PostUpdate};
use swamp_script::prelude::{quick_deserialize, ResolvedType, Value};
use tracing::info;

fn serialize(mut logic: LoReM<ScriptLogic>) {
    let mut buf = [0u8; 2048];
    let logic_val = logic.immutable_logic_value();
    if let Value::Struct(found_struct_type, _values) = &logic_val {
        let serialized_octet_size = logic_val.quick_serialize(&mut buf);
        info!(?serialized_octet_size, "serialized");

        let (deserialized_value, deserialized_octet_size) =
            quick_deserialize(&ResolvedType::Struct(found_struct_type.clone()), &buf);
        info!(?deserialized_octet_size, "deserialized_octet_size");

        logic.debug_set_logic_value(deserialized_value);

        //assert_eq!(serialized_octet_size, deserialized_octet_size);
    } else {
        panic!("logic has wrong type")
    }
}

pub struct SerializePlugin;

impl Plugin for SerializePlugin {
    fn build(&self, app: &mut App) {
        app.add_system(PostUpdate, serialize);
    }
}
