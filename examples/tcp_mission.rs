use futures::{future, SinkExt, StreamExt};
use nightingale::{
    component::Component,
    dialect::{
        MavAutopilot, MavModeFlag, MavState, MavType, Message, HEARTBEAT_DATA,
    },
    link::Link,
    mission::MissionItem::{Waypoint, Takeoff, ReturnToLaunch},
    wire::PacketCodec,
};
use tokio::net::TcpStream;
use tokio_util::codec::{FramedRead, FramedWrite};

const ADDR: &'static str = "127.0.0.1:5763";
const GCS_SYSTEM_ID: u8 = 255;
const GCS_COMPONENT_ID: u8 = 1;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Create a TCP connection, and split it into two halves.
    let connection = TcpStream::connect(ADDR).await?;
    let (reader, writer) = connection.into_split();

    // Create sink & streams for the link, ignore invalid packets with filter_map.
    let sink = FramedWrite::new(writer, PacketCodec);
    let stream = FramedRead::new(reader, PacketCodec)
        .filter_map(|result| future::ready(result.ok()));

    // Create a new link.
    let (link, connection) = Link::new(sink, stream, GCS_SYSTEM_ID, GCS_COMPONENT_ID);

    // Receive and broadcast heartbeat.
    let receive = receive_messages(link.clone());
    let broadcast = broadcast_heartbeat(link.clone());

    // Spawn background tasks.
    let tasks = tokio::spawn(future::join3(connection, receive, broadcast));

    // Create a component for drone's autopilot.
    let mut autopilot = Component::new(1, 1, link);

    let mission_items = [
        Waypoint(38.37061710, 27.20081034, 50.0),
        Takeoff(38.37061710, 27.20081034, 50.0),
        Waypoint(38.37052632, 27.20105989, 50.0),
        Waypoint(38.37066650, 27.20113415, 50.0),
        Waypoint(38.37089135, 27.20093708, 50.0),
        Waypoint(38.37086087, 27.20060531, 50.0),
        Waypoint(38.37053004, 27.20043123, 50.0),
        Waypoint(38.37034030, 27.20065871, 50.0),
        Waypoint(38.37037796, 27.20098516, 50.0),
        Waypoint(38.37052632, 27.20105989, 50.0),
        ReturnToLaunch,
    ];

    let result = autopilot.upload_mission(mission_items).await?;

    dbg!(result);

    let _ = tasks.await;

    Ok(())
}

async fn receive_messages(mut link: Link) {
    while let Some(p) = link.next().await {
        match p.message {
            Message::HEARTBEAT(_) => { eprintln!("HEARTBEAT sysid: {:?}, cmpid: {:?}", p.header.system_id, p.header.component_id) },
            _ => {}
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
            base_mode: MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED
                | MavModeFlag::MAV_MODE_FLAG_MANUAL_INPUT_ENABLED,
            mavlink_version: 3,
        });

        let _ = link.send(heartbeat).await;
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        eprintln!("[GCS] Heartbeat broadcasted.");
    }
}
