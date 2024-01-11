use nightingale::{
    component::Component,
    dialect::{
        MavMissionResult::MAV_MISSION_ACCEPTED as MissionAccepted,
        MavResult::MAV_RESULT_ACCEPTED as Accepted,
        *
    },
    link::Link,
    mission::MissionItem::{ReturnToLaunch, Takeoff, Waypoint},
    wire::PacketCodec,
};
use tokio::net::TcpStream;
use futures::{future, SinkExt, StreamExt};
use tokio_util::codec::{FramedRead, FramedWrite};
use std::time::Duration;

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
    let stream =
        FramedRead::new(reader, PacketCodec).filter_map(|result| future::ready(result.ok()));

    // Create a new link.
    let (link, connection) = Link::new(sink, stream, GCS_SYSTEM_ID, GCS_COMPONENT_ID);

    // Broadcast GCS hearbeat to the link.
    let broadcast = broadcast_heartbeat(link.clone());

    // Receive status text messages from the drone.
    let status = receive_status_text(link.clone());

    // Spawn connection and broadcast tasks.
    let tasks = tokio::spawn(future::join3(connection, broadcast, status));

    // Create a component for drone's autopilot.
    let mut autopilot = Component::new(1, 1, link);

    // Receive status messages (this is currently needed for wait_health_ok)
    eprintln!("Setting SYS_STATUS message rate...");
    match autopilot.set_message_interval(SYS_STATUS_DATA::ID, Duration::from_secs(2)).await? {
        Accepted => eprintln!("SYS_STATUS message interval set."),
        e => panic!("Couldn't set SYS_STATUS message interval [{e:?}], aborting..."),
    }

    // FIXME: Successful prearm checks does not mean that we can fly :(.

    // Wait until the drone is armable.
    eprintln!("Checking if the drone is armable...");
    if autopilot.wait_armable().await {
        eprintln!("The drone is currently armable.");
    } else {
        panic!("Couldn't receive armable, aborting...");
    }

    // Set mode to guided.
    eprintln!("Setting mode to GUIDED...");
    match autopilot.set_mode(CopterMode::COPTER_MODE_GUIDED).await? {
        Accepted => eprintln!("GUIDED mode set."),
        e => panic!("Couldn't set drone to GUIDED mode [{e:?}], aborting..."),
    }

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

    eprintln!("Uploading the mission...");
    match autopilot.upload_mission(mission_items).await? {
        MissionAccepted => eprintln!("The mission is accepted!"),
        e => panic!("The drone didn't accept the mission [{e:?}], aborting.."),
    }

    eprintln!("Arming the drone...");
    match autopilot.arm().await? {
        Accepted => eprintln!("Arm accepted, waiting for armed..."),
        e => panic!( "The drone didn't accept the command [{e:?}], aborting.."),
    }

    if autopilot.wait_armed().await {
        eprintln!("The drone is armed!");
    } else {
        eprintln!("Couldn't arm drone, aborting...");
    }

    eprintln!("Starting the mission.");
    match autopilot.start_mission().await? {
        Accepted => eprintln!("The mission has started!"),
        e => panic!("The drone didn't accept the command [{e:?}], aborting.."),
    }

    let _ = tasks.await;

    Ok(())
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
    }
}

async fn receive_status_text(link: Link) {
    link.for_each(|packet| async move {
        match &packet.message {
            Message::STATUSTEXT(STATUSTEXT_DATA { severity, text }) => {
                let content = std::str::from_utf8(text).expect("a valid utf8 string");
                eprintln!("[STATUS_TEXT] ({severity:?}) {content}");
            },
            _ => {}
        }
    }).await;
}
