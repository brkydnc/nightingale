use crate::{
    link::Link,
    dialect::{ MavAutopilot, MavModeFlag, MavState, MavComponent },
};

use std::sync::Arc;

struct System<T> {
    id: u8,
    state: MavState,
    mode: MavModeFlag,
    link: Arc<Link<T>>,
}

struct Component<T> {
    id: MavComponent,
    system: Arc<System<T>>,
    autopilot: MavAutopilot,
}

impl<T> Component<T> {
    fn new(id: MavComponent, autopilot: MavAutopilot, system: Arc<System<T>>) -> Self {
        Self { id, autopilot, system }
    }
}

// Command protocol implementation
impl<T> Component<T> {

}
