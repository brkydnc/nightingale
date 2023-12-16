use std::{ sync::Arc, time::Duration, ops::Deref };
use tokio::net::tcp::OwnedWriteHalf;
use tokio_util::codec::{FramedRead, FramedWrite};
use nightingale::{
    link::{Link, Subscriber},
    wire::{PacketDecoder, PacketEncoder, Packet},
    error::{Result, Error},
    dialect::{
        Message,
        MavType,
        MavFrame,
        MavAutopilot,
        MavState,
        MavModeFlag,
        MavCmd,
        MessageExt,
        MavMissionResult,
        MISSION_ITEM_INT_DATA as RawMissionItem,
        MISSION_COUNT_DATA,
        HEARTBEAT_DATA,
        COMMAND_INT_DATA,
        COMMAND_LONG_DATA,
    }
};

const GCS_SYSTEM_ID: u8 = 255;
const GCS_COMPONENT_ID: u8 = 1;
const TARGET_SYSTEM_ID: u8 = 1;
const TARGET_COMPONENT_ID: u8 = 1;

type TcpLink = Link<FramedWrite<OwnedWriteHalf, PacketEncoder>>;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let connection = tokio::net::TcpStream::connect("127.0.0.1:5762").await?;
    let (reader, writer) = connection.into_split();

    let sink = FramedWrite::new(writer, PacketEncoder);
    let stream = FramedRead::new(reader, PacketDecoder);

    let link = Arc::new(Link::new(sink, stream));

    let receive = tokio::spawn(receive_messages(link.subscribe()));
    let broadcast = tokio::spawn(broadcast_heartbeat(link.clone()));

    // Receive GLOBAL_POSITION_INT every  seconds
    set_message_interval(link.clone(), 33, Duration::from_secs(1)).await;

    let planner = MissionPlanner::new()
        .add(MissionItem::Waypoint(38.37061710, 27.20081034, 50.0))
        .add(MissionItem::Takeoff(38.37061710, 27.20081034, 50.0))
        .add(MissionItem::Waypoint(38.37052632, 27.20105989, 50.0))
        .add(MissionItem::Waypoint(38.37066650, 27.20113415, 50.0))
        .add(MissionItem::Waypoint(38.37089135, 27.20093708, 50.0))
        .add(MissionItem::Waypoint(38.37086087, 27.20060531, 50.0))
        .add(MissionItem::Waypoint(38.37053004, 27.20043123, 50.0))
        .add(MissionItem::Waypoint(38.37034030, 27.20065871, 50.0))
        .add(MissionItem::Waypoint(38.37037796, 27.20098516, 50.0))
        .add(MissionItem::Waypoint(38.37052632, 27.20105989, 50.0))
        .add(MissionItem::ReturnToLaunch);

    let result = planner.upload(link.clone()).await?;

    match result {
        MavMissionResult::MAV_MISSION_ACCEPTED => {
            eprintln!("mission accepted");

            let arm = Message::COMMAND_LONG(COMMAND_LONG_DATA {
                param1: 1.0,
                param2: 0.0,
                command: MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                target_system: TARGET_SYSTEM_ID,
                target_component: TARGET_COMPONENT_ID,
                ..Default::default()
            });

            eprintln!("armed");

            let _ = link.send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, arm).await;

            let mission_start = Message::COMMAND_LONG(COMMAND_LONG_DATA {
                param1: 0.0,
                param2: 0.0,
                command: MavCmd::MAV_CMD_MISSION_START,
                target_system: TARGET_SYSTEM_ID,
                target_component: TARGET_COMPONENT_ID,
                ..Default::default()
            });

            let _ = link.send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, mission_start).await;

            eprintln!("started");
        },
        _ => { dbg!(result); },
    }

    let _ = tokio::join!(receive, broadcast);

    Ok(())
}

async fn receive_messages(mut subscriber: Subscriber) {
    while let Ok(p) = subscriber.wait_for(&mut |_| true).await {
        match p.message {
            // 0 => { eprintln!("HEARTBEAT sysid: {:?}, cmpid: {:?}", p.header.system_id, p.header.component_id) },
            // 51 | 47 => { eprintln!("{:#?}", p.message) },
            Message::GLOBAL_POSITION_INT(pos) => {
                eprintln!("lat: {}, lon: {}, alt: {}", pos.lat, pos.lon, pos.alt);
            },
            _ => { }
        }
    }
}

async fn broadcast_heartbeat(link: Arc<TcpLink>) {
    loop {
        let heartbeat = Message::HEARTBEAT(HEARTBEAT_DATA {
            custom_mode: 0,
            mavtype: MavType::MAV_TYPE_GCS,
            autopilot: MavAutopilot::MAV_AUTOPILOT_INVALID,
            system_status: MavState::MAV_STATE_ACTIVE,
            base_mode: MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED | MavModeFlag::MAV_MODE_FLAG_MANUAL_INPUT_ENABLED,
            mavlink_version: 3,
        });

        let _ = link.send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, heartbeat).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn set_message_interval(link: Arc<TcpLink>, message: u32, interval: Duration) {
    let command = Message::COMMAND_INT(COMMAND_INT_DATA {
        command: MavCmd::MAV_CMD_SET_MESSAGE_INTERVAL,
        target_system: 1,
        target_component: 0,
        param1: message as f32,
        param2: interval.as_micros() as f32,
        ..Default::default()
    });

    let _ = link.send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, command).await;
}

enum MissionItem {
    Waypoint(f32, f32, f32),
    Takeoff(f32, f32, f32),
    ReturnToLaunch,
}

impl MissionItem {
    fn raw(self) -> RawMissionItem {
        use MissionItem::*;

        fn scale(f: f32) -> i32 { (f * 1e7) as i32 }

        match self {
            Waypoint(lat, lon, alt) => RawMissionItem {
                command: MavCmd::MAV_CMD_NAV_WAYPOINT,
                param4: f32::NAN,
                x: scale(lat),
                y: scale(lon),
                z: alt,
                autocontinue: true as u8,
                target_system: TARGET_SYSTEM_ID,
                target_component: TARGET_COMPONENT_ID,
                frame: MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT_INT,
                ..Default::default()
            },
            Takeoff(lat, lon, alt) => RawMissionItem {
                command: MavCmd::MAV_CMD_NAV_TAKEOFF,
                param4: f32::NAN,
                x: scale(lat),
                y: scale(lon),
                z: alt,
                autocontinue: true as u8,
                target_system: TARGET_SYSTEM_ID,
                target_component: TARGET_COMPONENT_ID,
                frame: MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT_INT,
                ..Default::default()
            },
            ReturnToLaunch => RawMissionItem {
                command: MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
                autocontinue: true as u8,
                target_system: TARGET_SYSTEM_ID,
                target_component: TARGET_COMPONENT_ID,
                frame: MavFrame::MAV_FRAME_MISSION,
                ..Default::default()
            }
        }
    }
}

struct MissionPlanner {
    items: Vec<RawMissionItem>,
}

impl MissionPlanner {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    fn add(mut self, item: MissionItem) -> Self {
        let mut raw = item.raw();
        raw.seq = self.items.len() as u16;
        self.items.push(raw);
        self
    }

    async fn upload(&self, link: Arc<TcpLink>) -> Result<MavMissionResult> {
        let mission_count = Message::MISSION_COUNT(MISSION_COUNT_DATA {
            count: self.items.len() as u16,
            target_system: TARGET_SYSTEM_ID,
            target_component: TARGET_COMPONENT_ID,
        });

        let mut subscriber = link.subscribe();

        let retries = 5;
        let duration = Duration::from_millis(1500);
        let capture_ack_or_req = &mut |p: &Packet| {
            let id = p.message.message_id();
            // Receive MISSION_REQUEST_INT or MISSION_REQUEST (deprecated )or MISSION_ACK
            id == 51 || id == 40 || id == 47
        };

        link
            .clone()
            .spawn_send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, mission_count);

        let mission_result = loop {
            // XXX: This does not see simultaneous sends.
            let packet = subscriber
                .timeout_for(capture_ack_or_req, duration, retries)
                .await
                .ok_or(Error::Timeout)??;

            match packet.message {
                Message::MISSION_REQUEST(req) => {
                    // TODO: Handle invalid seq (seq < items.len());
                    let data = &self.items[req.seq as usize];
                    let mission_item = Message::MISSION_ITEM_INT(data.clone());

                    link
                        .clone()
                        .spawn_send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, mission_item);
                },
                Message::MISSION_REQUEST_INT(req) => {
                    // TODO: Handle invalid seq (seq < items.len());
                    let data = &self.items[req.seq as usize];
                    let mission_item = Message::MISSION_ITEM_INT(data.clone());

                    link
                        .clone()
                        .spawn_send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, mission_item);
                },
                Message::MISSION_ACK(ack) => {
                    break ack.mavtype;
                },
                _ => unreachable!(),
            }
        };

        Ok(mission_result)
    }
}
