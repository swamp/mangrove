use crate::logic::ScriptLogic;
use limnus_gamepad::{Axis, Button, GamepadMessage};
use swamp::prelude::{App, FixedPostUpdate, LoRe, LoReM, LocalResource, Msg, Plugin, PostUpdate};
use swamp_script::prelude::{quick_deserialize, ResolvedType, Value};
use tracing::info;

fn serialize(mut logic: LoReM<ScriptLogic>, mut rewind: LoReM<Rewind>) {
    let mut buf = [0u8; 2048];
    let logic_val = logic.immutable_logic_value();
    if let Value::Struct(found_struct_type, _values) = &logic_val {
        if rewind.tick_to_show.is_none() {
            let serialized_octet_size = logic_val.quick_serialize(&mut buf, 0);
            rewind.snapshots.push(Snapshot {
                payload: buf[0..serialized_octet_size].to_vec(),
            });

            // Just for verification
            {
                let (_deserialized_value, deserialized_octet_size) =
                    quick_deserialize(&ResolvedType::Struct(found_struct_type.clone()), &buf, 0);

                assert_eq!(serialized_octet_size, deserialized_octet_size);
            }
        }

        let index_to_show = {
            match rewind.tick_to_show {
                None => rewind.snapshots.len() - 1,
                Some(index) => index.clamp(0, rewind.snapshots.len() - 1),
            }
        };

        let payload = &rewind.snapshots[index_to_show].payload;

        let (deserialized_payload_value, _deserialized_octet_size) =
            quick_deserialize(&ResolvedType::Struct(found_struct_type.clone()), payload, 0);

        logic.debug_set_logic_value(deserialized_payload_value);
    } else {
        panic!("logic has wrong type")
    }
}

fn set_velocity(rewind: &mut Rewind, new_velocity: f32) {
    if let Some(ref mut velocity) = rewind.tick_velocity {
        *velocity = (new_velocity * new_velocity) * 5.0 * new_velocity.signum();
        info!(index=%*velocity, "new_velocity");
    } else {
        rewind.tick_velocity = Some(new_velocity);
        rewind.tick_float = (rewind.snapshots.len() - 1) as f32;
    };
}

fn change_tick(mut rewind: LoReM<Rewind>) {
    if let Some(found_velocity) = rewind.tick_velocity {
        rewind.tick_float += found_velocity;
        rewind.tick_float = rewind
            .tick_float
            .clamp(0.0, (rewind.snapshots.len() - 1) as f32);
        rewind.tick_to_show = Some(rewind.tick_float as usize)
    }
}

fn rewind(messages: Msg<GamepadMessage>, mut rewind: LoReM<Rewind>) {
    for gamepad in messages.iter_current() {
        match gamepad {
            GamepadMessage::Connected(_, _) => {}
            GamepadMessage::Disconnected(_) => {}
            GamepadMessage::Activated(_) => {}
            GamepadMessage::ButtonChanged(gamepad_id, button, button_value) => match button {
                Button::South => {}
                Button::East => {}
                Button::North => {}
                Button::West => {}
                Button::LeftTrigger => {}
                Button::LeftTrigger2 => {
                    set_velocity(&mut rewind, -*button_value);
                }
                Button::RightTrigger => {}
                Button::RightTrigger2 => {
                    set_velocity(&mut rewind, *button_value);
                }
                Button::Select => {
                    rewind.tick_to_show = None;
                    rewind.tick_velocity = None;
                }
                Button::Start => {}
                Button::Mode => {}
                Button::LeftThumb => {}
                Button::RightThumb => {}
                Button::DPadUp => {}
                Button::DPadDown => {}
                Button::DPadLeft => {}
                Button::DPadRight => {}
            },
            GamepadMessage::AxisChanged(gamepad_id, axis, value) => match axis {
                Axis::LeftStickX => {}
                Axis::LeftStickY => {}
                Axis::RightStickX => {}
                Axis::RightStickY => {}
            },
        }
    }
}

#[derive(Debug)]
pub struct Snapshot {
    pub payload: Vec<u8>,
}

#[derive(Debug, LocalResource)]
pub struct Rewind {
    pub tick_velocity: Option<f32>,
    pub tick_float: f32,
    pub snapshots: Vec<Snapshot>,
    pub tick_to_show: Option<usize>,
}

pub struct SerializePlugin;

impl Plugin for SerializePlugin {
    fn build(&self, app: &mut App) {
        app.insert_local_resource(Rewind {
            tick_velocity: None,
            tick_to_show: None,
            tick_float: 0.0,
            snapshots: vec![],
        });
        app.add_system(FixedPostUpdate, serialize);
        app.add_system(FixedPostUpdate, rewind);
        app.add_system(FixedPostUpdate, change_tick);
    }
}
