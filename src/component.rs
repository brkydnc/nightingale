use crate::{
    dialect::{
        MavComponent, MavMissionResult, Message, MISSION_COUNT_DATA,
        MISSION_ITEM_DATA as RawMissionItem,
    },
    error::Result,
    link::Link,
    wire::Packet,
};
use std::time::Duration;

pub struct Component {
    id: u8,
    system: u8,
    link: Link,
}

impl Component {
    pub fn new(component: MavComponent, system: u8, link: Link) -> Self {
        Self {
            id: component as u8,
            system,
            link,
        }
    }

    pub async fn upload_mission(&mut self, items: &[RawMissionItem]) -> Result<MavMissionResult> {
        use Message::{
            MISSION_ACK as Ack, MISSION_ITEM as Item, MISSION_ITEM as ItemInt,
            MISSION_REQUEST as Request, MISSION_REQUEST_INT as RequestInt,
        };

        let filter = |p: &Packet| matches!(p.message, Request(_) | RequestInt(_) | Ack(_));

        let mission_count = Message::MISSION_COUNT(MISSION_COUNT_DATA {
            count: items.len() as u16,
            target_system: self.system,
            target_component: self.id,
        });

        self.link.send_message(mission_count).await?;

        let mission_result = loop {
            let packet = self
                .link
                .timeout(filter, Duration::from_millis(1500), 5)
                .await?;

            match &packet.message {
                Request(req) => {
                    let seq = req.seq as usize;
                    let mission_item = Item(items[seq].clone());
                    self.link.send_message(mission_item).await?;
                }
                RequestInt(req) => {
                    let seq = req.seq as usize;
                    let mission_item = ItemInt(items[seq].clone());
                    self.link.send_message(mission_item).await?;
                }
                Ack(ack) => {
                    break ack.mavtype;
                }
                _ => unreachable!(),
            }
        };

        Ok(mission_result)
    }
}
