use std::time::Duration;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio::net::TcpStream;
use futures::{future, SinkExt, StreamExt};
use nightingale::{
    link::Link,
    wire::PacketCodec,
    mission::{MissionPlanner, MissionItem},
    dialect::{
        Message,
        MavType,
        MavAutopilot,
        MavState,
        MavModeFlag,
        MavCmd,
        HEARTBEAT_DATA,
        COMMAND_INT_DATA,
    }
};

const ADDR: &'static str = "127.0.0.1:5763";
const SYSTEM_ID: u8 = 255;
const COMPONENT_ID: u8 = 1;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let connection = TcpStream::connect(ADDR).await?;
    let (reader, writer) = connection.into_split();

    let sink = FramedWrite::new(writer, PacketCodec);
    let stream = FramedRead::new(reader, PacketCodec)
        .filter_map(|result| future::ready(result.ok()));

    let (mut link, fut) = Link::new(sink, stream, SYSTEM_ID, COMPONENT_ID);

    let connection = tokio::spawn(fut);
    let receive = tokio::spawn(receive_messages(link.clone()));
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

    // let result = planner.upload(link.clone()).await?;

    // match result {
    //     MavMissionResult::MAV_MISSION_ACCEPTED => {
    //         eprintln!("mission accepted");

    //         let arm = Message::COMMAND_LONG(COMMAND_LONG_DATA {
    //             param1: 1.0,
    //             param2: 0.0,
    //             command: MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
    //             target_system: TARGET_SYSTEM_ID,
    //             target_component: TARGET_COMPONENT_ID,
    //             ..Default::default()
    //         });

    //         eprintln!("armed");

    //         let _ = link.send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, arm).await;

    //         let mission_start = Message::COMMAND_LONG(COMMAND_LONG_DATA {
    //             param1: 0.0,
    //             param2: 0.0,
    //             command: MavCmd::MAV_CMD_MISSION_START,
    //             target_system: TARGET_SYSTEM_ID,
    //             target_component: TARGET_COMPONENT_ID,
    //             ..Default::default()
    //         });

    //         let _ = link.send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, mission_start).await;

    //         eprintln!("started");
    //     },
    //     _ => { dbg!(result); },
    // }

    // let _ = tokio::join!(receive, broadcast);

    Ok(())
}

async fn receive_messages(mut link: Link) {
    while let Some(p) = link.next().await {
        match p.message {
            // Message::HEARTBEAT(_) => { eprintln!("HEARTBEAT sysid: {:?}, cmpid: {:?}", p.header.system_id, p.header.component_id) },
            // Message::GLOBAL_POSITION_INT(pos) => { eprintln!("lat: {}, lon: {}, alt: {}", pos.lat, pos.lon, pos.alt); },
            _ => { }
        }
    }
}

async fn broadcast_heartbeat(mut link: Link) {
    loop {
        let heartbeat = Message::HEARTBEAT(HEARTBEAT_DATA {
            custom_mode: 0,
            mavtype: MavType::MAV_TYPE_GCS,
            autopilot: MavAutopilot::MAV_AUTOPILOT_INVALID,
            system_status: MavState::MAV_STATE_ACTIVE,
            base_mode: MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED | MavModeFlag::MAV_MODE_FLAG_MANUAL_INPUT_ENABLED,
            mavlink_version: 3,
        });

        let _ = link.send(heartbeat).await;
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        eprintln!("[GCS] Heartbeat broadcasted.");
    }
}

async fn set_message_interval(mut link: Link, message: u32, interval: Duration) {
    let command = Message::COMMAND_INT(COMMAND_INT_DATA {
        command: MavCmd::MAV_CMD_SET_MESSAGE_INTERVAL,
        target_system: 1,
        target_component: 0,
        param1: message as f32,
        param2: interval.as_micros() as f32,
        ..Default::default()
    });

    let _ = link.send(command).await;
}
