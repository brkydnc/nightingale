use crate::{
    link::Link,
    dialect::{
        Message,
        MavComponent,
        MavMissionResult,
        MISSION_COUNT_DATA,
        MISSION_ITEM_DATA as RawMissionItem,
    },
};

struct Component {
    id: u8,
    system: u8,
    link: Link,
}

impl Component {
    fn new(component: MavComponent, system: u8, link: Link) -> Self {
        Self { id: component as u8, system, link }
    }

    fn upload_mission(&mut self, items: &[RawMissionItem]) {
        // let mission_count = Message::MISSION_COUNT(MISSION_COUNT_DATA {
        //     count: items.len() as u16,
        //     target_system: self.system,
        //     target_component: self.id,
        // });

        // let retries = 5;
        // let duration = Duration::from_millis(1500);
        // let capture_ack_or_req = &mut |p: &Packet| {
        //     let id = p.message.message_id();
        //     // Receive MISSION_REQUEST_INT or MISSION_REQUEST (deprecated )or MISSION_ACK
        //     id == 51 || id == 40 || id == 47
        // };

        // link
        //     .clone()
        //     .spawn_send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, mission_count);

        // let mission_result = loop {
        //     // XXX: This does not see simultaneous sends.
        //     let packet = subscriber
        //         .timeout_for(capture_ack_or_req, duration, retries)
        //         .await
        //         .ok_or(Error::Timeout)??;

        //     match packet.message {
        //         Message::MISSION_REQUEST(req) => {
        //             // TODO: Handle invalid seq (seq < items.len());
        //             let data = &self.items[req.seq as usize];
        //             let mission_item = Message::MISSION_ITEM_INT(data.clone());

        //             link
        //                 .clone()
        //                 .spawn_send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, mission_item);
        //         },
        //         Message::MISSION_REQUEST_INT(req) => {
        //             // TODO: Handle invalid seq (seq < items.len());
        //             let data = &self.items[req.seq as usize];
        //             let mission_item = Message::MISSION_ITEM_INT(data.clone());

        //             link
        //                 .clone()
        //                 .spawn_send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, mission_item);
        //         },
        //         Message::MISSION_ACK(ack) => {
        //             break ack.mavtype;
        //         },
        //         _ => unreachable!(),
        //     }
        // };

        // Ok(mission_result)
    }
}
