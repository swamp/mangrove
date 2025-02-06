/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/limnus
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */

use limnus_app::prelude::{App, Plugin};
use limnus_resource::prelude::Resource;
use std::collections::HashMap;
use tracing::{debug, error, trace};

pub type ControllerId = usize;

#[derive(Debug, Default, Clone, Copy)]
pub struct AxisValueType {
    x: f32,
    y: f32,
}
pub type ButtonValueType = bool;

#[derive(Default, Debug, Clone)]
pub struct Controller {
    pub axis: [AxisValueType; 4],
    pub buttons: [ButtonValueType; 32], // TODO: Fix a bitmask for the buttons
    pub id: ControllerId,
    pub is_active: bool,
}

impl Controller {
    #[must_use]
    pub fn new(id: ControllerId) -> Self {
        Self {
            axis: [AxisValueType::default(); 4],
            buttons: [false; 32],
            id,
            is_active: false,
        }
    }

    #[must_use]
    pub const fn axis(&self, index: usize) -> AxisValueType {
        self.axis[index]
    }

    #[must_use]
    pub const fn button(&self, index: usize) -> ButtonValueType {
        self.buttons[index]
    }
}

#[derive(Debug, Resource)]
pub struct Controllers {
    gamepads: HashMap<ControllerId, Controller>,
}

impl Default for Controllers {
    fn default() -> Self {
        Self::new()
    }
}

impl Controllers {
    /// Creates a new Controller instance
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this gamepad
    /// * `name` - Human-readable name of the gamepad
    pub fn new() -> Self {
        Self {
            gamepads: HashMap::new(),
        }
    }
    pub fn connected(
        &mut self,
        id: ControllerId,
        name: &str,
        queue: &mut Messages<ControllerMessage>,
    ) {
        debug!(id=%id, name=name, "connected gamepad");
        self.gamepads.insert(id, Controller::new(id, name));
        queue.send(ControllerMessage::Connected(id, name.to_string()));
    }

    pub fn disconnected(&mut self, id: ControllerId, queue: &mut Messages<ControllerMessage>) {
        if let Some(existing) = self.gamepads.remove(&id) {
            debug!(id=%id, name=?existing.name, "disconnected gamepad");
            queue.send(ControllerMessage::Disconnected(id));
        } else {
            error!(id=%id, "gamepad not found");
        }
    }

    #[must_use]
    pub fn controller(&self, id: ControllerId) -> Option<&Controller> {
        self.gamepads.get(&id)
    }

    /// Gets the axis value for a gamepad
    #[must_use]
    pub fn axis(&self, id: ControllerId, index: usize) -> Option<AxisValueType> {
        self.gamepad(id).map(|pad| pad.axis[index as usize])
    }

    /// Gets the button value for a gamepad
    #[must_use]
    pub fn button(&self, id: ControllerId, index: usize) -> Option<ButtonValueType> {
        self.gamepad(id).map(|pad| pad.buttons[index as usize])
    }

    pub fn iter_active(&self) -> impl Iterator<Item = &Controller> {
        self.gamepads.values().filter(|gamepad| gamepad.is_active)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Controller> {
        self.gamepads.values()
    }

    pub fn set_axis(
        &mut self,
        id: ControllerId,
        axis: usize,
        value: AxisValueType,
        queue: &mut Messages<ControllerMessage>,
    ) -> Option<()> {
        trace!(id=?id, axis=?axis, value=?value, "set axis");
        let gamepad = self.gamepads.get_mut(&id)?;

        queue.send(ControllerMessage::AxisChanged(id, axis, value));
        gamepad.axis[axis as usize] = value;

        Some(())
    }

    pub fn set_button(
        &mut self,
        id: ControllerId,
        button: usize,
        value: ButtonValueType,
        queue: &mut Messages<ControllerMessage>,
    ) -> Option<()> {
        trace!(id=?id, button=?button, value=?value, "set button");

        let gamepad = self.gamepads.get_mut(&id)?;

        if !gamepad.is_active && value > 0.1 {
            debug!(id=%id, button=?button, name=%gamepad.name, "gamepad activated");
            queue.send(ControllerMessage::Activated(id));
            gamepad.is_active = true;
        }

        queue.send(ControllerMessage::ButtonChanged(id, button, value));
        gamepad.buttons[button as usize] = value;
        Some(())
    }

    /// Gets the name of a gamepad
    #[must_use]
    pub fn name(&self, id: ControllerId) -> Option<&str> {
        self.gamepad(id).map(|pad| pad.name.as_str())
    }
}

pub struct ControllerResourcePlugin;

impl Plugin for ControllerResourcePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Controllers::new());
        app.create_message_type::<ControllerMessage>();
    }
}
