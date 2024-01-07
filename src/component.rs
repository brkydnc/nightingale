use std::time::Duration;
use crate::{
    dialect::{MavMissionResult, Message, MISSION_COUNT_DATA },
    error::Result,
    link::Link,
    wire::Packet,
    mission::IntoMissionItem,
};

pub struct Component {
    id: u8,
    system: u8,
    link: Link,
}

impl Component {
    pub fn new(id: u8, system: u8, link: Link) -> Self {
        Self { id, system, link }
    }

    pub fn id(&self) -> u8 {
        self.id
    }

    pub fn system(&self) -> u8 {
        self.system
    }

    pub async fn upload_mission<M, I>(
        &mut self,
        mission: M,
    ) -> Result<MavMissionResult>
        where M: AsRef<[I]>,
              I: IntoMissionItem,
    {
        use Message::{
            MISSION_ACK as Ack, MISSION_ITEM as Item, MISSION_ITEM_INT as ItemInt,
            MISSION_REQUEST as Request, MISSION_REQUEST_INT as RequestInt,
        };

        let filter = |p: &Packet| matches!(p.message, Request(_) | RequestInt(_) | Ack(_));
        let items = mission.as_ref();

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
                    let item = items[req.seq as usize]
                        .with(self.system, self.id, req.seq);

                    let mission_item = Item(item);
                    self.link.send_message(mission_item).await?;
                }
                RequestInt(req) => {
                    let item = items[req.seq as usize]
                        .with_int(self.system, self.id, req.seq);

                    let mission_item = ItemInt(item);
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
