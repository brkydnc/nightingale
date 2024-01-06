pub mod core;
pub mod component;

pub mod dialect {
    pub use mavlink::{MavHeader as Header, Message as MessageExt};

    pub use mavlink::ardupilotmega::{MavMessage as Message, *};

    pub type MessageId = u32;
}

pub use core::*;
