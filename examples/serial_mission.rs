use futures::{future, SinkExt, StreamExt};
use tokio_util::codec::Decoder;
use tokio_serial::SerialPortBuilderExt;
use nightingale::{
    component::Component,
    dialect::{
        MavCmd, MavMissionResult::MAV_MISSION_ACCEPTED as MissionAccepted,
        MavResult::MAV_RESULT_ACCEPTED as Accepted, COMMAND_LONG_DATA as CommandLong,
    },
    link::Link,
    mission::MissionItem::{ReturnToLaunch, Takeoff, Waypoint},
    wire::PacketCodec,
};

const GCS_SYSTEM_ID: u8 = 255;
const GCS_COMPONENT_ID: u8 = 1;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Create a Serial connection, and split it into two halves.
    let port = tokio_serial::new("/dev/ttyUSB0", 57600).open_native_async()?;
    let (sink, stream) = PacketCodec.framed(port).split();

    // Ignore invalid packets.
    let stream = stream.filter_map(|result| future::ready(result.ok()));

    // Create a new link.
    let (link, connection) = Link::new(sink, stream, GCS_SYSTEM_ID, GCS_COMPONENT_ID);

    // Broadcast GCS hearbeat to the link.
    let broadcast = broadcast_heartbeat(link.clone());

    // Spawn connection and broadcast tasks.
    let tasks = tokio::spawn(future::join(connection, broadcast));

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

    eprintln!("Uploading the mission...");
    let mission_result = autopilot.upload_mission(mission_items).await?;

    match mission_result {
        MissionAccepted => eprintln!("The mission is accepted!"),
        reason => panic!(
            "The drone didn't accept the mission [{:?}], aborting..",
            reason
        ),
    }

    let arm = CommandLong {
        param1: 1.0,
        command: MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
        ..Default::default()
    };

    eprintln!("Arming the drone...");
    let arm_result = autopilot.command_long(arm).await?;

    match arm_result {
        Accepted => eprintln!("The drone is armed!"),
        reason => panic!(
            "The drone didn't accept the command [{:?}], aborting..",
            reason
        ),
    }

    let start = CommandLong {
        command: MavCmd::MAV_CMD_MISSION_START,
        ..Default::default()
    };

    eprintln!("Starting the mission.");
    let start_result = autopilot.command_long(start).await?;

    match start_result {
        Accepted => eprintln!("The mission has started!"),
        reason => panic!(
            "The drone didn't accept the command [{:?}], aborting..",
            reason
        ),
    }

    let _ = tasks.await;

    Ok(())
}

async fn broadcast_heartbeat(mut link: Link) {
    use nightingale::dialect::{
        MavAutopilot, MavModeFlag, MavState, MavType, Message, HEARTBEAT_DATA,
    };

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
