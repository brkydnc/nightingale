// pub mod command;
pub mod error;
pub mod link;
pub mod wire;

pub mod dialect {
    pub use mavlink::{MavHeader as Header, Message as MessageExt};

    pub use mavlink::ardupilotmega::{MavMessage as Message, *};

    pub type MessageId = u32;
}
