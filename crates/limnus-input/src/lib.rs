/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/swamp/limnus
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */

use limnus_app::prelude::{App, Plugin};
use limnus_message::prelude::Message;
use limnus_message::Messages;
use limnus_resource::prelude::Resource;
use std::collections::HashMap;
use tracing::{debug, error, trace};

#[derive(Debug, Message)]
pub enum ControllerMessage {
    Connected(ControllerId, String),
    Disconnected(ControllerId),
    Activated(ControllerId), // Sent when first button is pressed
}

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
    controllers: HashMap<ControllerId, Controller>,
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
    /// * `id` - Unique identifier for this controller
    /// * `name` - Human-readable name of the controller
    #[must_use]
    pub fn new() -> Self {
        Self {
            controllers: HashMap::new(),
        }
    }
    pub fn connected(
        &mut self,
        id: ControllerId,
        name: &str,
        queue: &mut Messages<ControllerMessage>,
    ) {
        debug!(id=%id, name=name, "connected controller");
        self.controllers.insert(id, Controller::new(id));
        queue.send(ControllerMessage::Connected(id, name.to_string()));
    }

    pub fn disconnected(&mut self, id: ControllerId, queue: &mut Messages<ControllerMessage>) {
        if let Some(existing) = self.controllers.remove(&id) {
            queue.send(ControllerMessage::Disconnected(id));
        } else {
            error!(id=%id, "controller not found");
        }
    }

    #[must_use]
    pub fn controller(&self, id: ControllerId) -> Option<&Controller> {
        self.controllers.get(&id)
    }

    /// Gets the axis value for a controller
    #[must_use]
    pub fn axis(&self, id: ControllerId, index: usize) -> Option<AxisValueType> {
        self.controller(id).map(|pad| pad.axis[index as usize])
    }

    /// Gets the button value for a controller
    #[must_use]
    pub fn button(&self, id: ControllerId, index: usize) -> Option<ButtonValueType> {
        self.controller(id).map(|pad| pad.buttons[index as usize])
    }

    pub fn iter_active(&self) -> impl Iterator<Item = &Controller> {
        self.controllers
            .values()
            .filter(|controller| controller.is_active)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Controller> {
        self.controllers.values()
    }

    pub fn set_axis(&mut self, id: ControllerId, axis: usize, value: AxisValueType) -> Option<()> {
        trace!(id=?id, axis=?axis, value=?value, "set axis");
        let controller = self.controllers.get_mut(&id)?;

        controller.axis[axis] = value;

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

        let controller = self.controllers.get_mut(&id)?;

        if !controller.is_active && value {
            //debug!(id=%id, button=?button, name=%controller.name, "controller activated");
            queue.send(ControllerMessage::Activated(id));
            controller.is_active = true;
        }

        controller.buttons[button] = value;
        Some(())
    }
}

pub struct ControllerResourcePlugin;

impl Plugin for ControllerResourcePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Controllers::new());
        app.create_message_type::<ControllerMessage>();
    }
}
