use crate::logic::ScriptLogic;
use swamp::prelude::{App, LoReM, Plugin, PostUpdate};
use swamp_script::prelude::{quick_deserialize, ResolvedType, Value};

fn serialize(mut logic: LoReM<ScriptLogic>) {
    let mut buf = [0u8; 2048];
    let logic_val = logic.immutable_logic_value();
    if let Value::Struct(found_struct_type, _values) = &logic_val {
        let serialized_octet_size = logic_val.quick_serialize(&mut buf, 0);

        let (deserialized_value, deserialized_octet_size) =
            quick_deserialize(&ResolvedType::Struct(found_struct_type.clone()), &buf, 0);

        logic.debug_set_logic_value(deserialized_value);

        assert_eq!(serialized_octet_size, deserialized_octet_size);
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
