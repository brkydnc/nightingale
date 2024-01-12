use nightingale::{
    dialect::{
        MavMissionResult::MAV_MISSION_ACCEPTED as MissionAccepted,
        MavResult::MAV_RESULT_ACCEPTED as Accepted,
        *
    },
    link::Link,
    mission::MissionItem::{ReturnToLaunch, Takeoff, Waypoint},
    error::Error,
    wire::{Packet, PacketCodec},
    component::Component,
};
use std::{net::SocketAddr, time::Duration, io::Error as IoError, future::Future};
use tokio::net::UdpSocket;
use tokio_util::udp::UdpFramed;
use futures::{future, SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_util::codec::{FramedRead, FramedWrite, Decoder};
use tokio_serial::SerialPortBuilderExt;

const GCS_SYSTEM_ID: u8 = 255;
const GCS_COMPONENT_ID: u8 = 1;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Create a new link.
    // let (link, connection) = udp("0.0.0.0:14550").await?;
    let (link, connection) = serial("/dev/cu.usbserial-0001", 57600).await?;
    // let (link, connection) = tcp("127.0.0.1:5763").await?;

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

    // // Wait until the drone is armable.
    // eprintln!("Waiting armable...");
    // if autopilot.wait_armable().await {
    //     eprintln!("The drone is currently armable.");
    // } else {
    //     panic!("Couldn't receive armable, aborting...");
    // }

    // Set mode to guided.
    let mode = CopterMode::COPTER_MODE_ACRO;
    eprintln!("Setting mode to {mode:?}...");
    match autopilot.set_mode(mode).await? {
        Accepted => eprintln!("{mode:?} mode set."),
        e => panic!("Couldn't set drone to {mode:?} mode [{e:?}], aborting..."),
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
        Accepted => eprintln!("Arm accepted."),
        e => panic!( "The drone didn't accept the command [{e:?}], aborting.."),
    }

    eprintln!("Waiting for armed...");
    if autopilot.wait_armed().await {
        eprintln!("The drone is currently armed.");
    } else {
        panic!("Couldn't wait for armed, aborting...");
    }

    // eprintln!("Starting the mission.");
    // match autopilot.start_mission().await? {
    //     Accepted => eprintln!("The mission has started!"),
    //     e => panic!("The drone didn't accept the command [{e:?}], aborting.."),
    // }

    let _ = tasks.await;

    Ok(())
}

async fn udp(bind: &str) -> Result<(Link, impl Future<Output = ()>), IoError> {
    // Create a UDP connection, and split it into two halves.
    let socket = UdpSocket::bind(bind).await?;
    let (sink, stream) = UdpFramed::new(socket, PacketCodec).split();

    // Create a socket address from the address string.
    let address: SocketAddr = "192.168.4.1:14555".parse().unwrap();

    // Opt out addresses in sink and stream.
    let sink = sink.with(move |packet: Packet| {
        future::ok::<(Packet, SocketAddr), Error>((packet, address))
    });

    let stream = stream.filter_map(|result| {
        future::ready(result.ok().map(|(packet, _)| packet))
    });

    Ok(Link::new(sink, stream, GCS_SYSTEM_ID, GCS_COMPONENT_ID))
}

async fn tcp(address: &str) -> Result<(Link, impl Future<Output = ()>), IoError> {
    // Create a TCP connection, and split it into two halves.
    let connection = TcpStream::connect(address).await?;
    let (reader, writer) = connection.into_split();

    // Create sink & streams for the link, ignore invalid packets with filter_map.
    let sink = FramedWrite::new(writer, PacketCodec);
    let stream = FramedRead::new(reader, PacketCodec)
        .filter_map(|result| future::ready(result.ok()));

    Ok(Link::new(sink, stream, GCS_SYSTEM_ID, GCS_COMPONENT_ID))
}

async fn serial(path: &str, baud: u32) -> Result<(Link, impl Future<Output = ()>), IoError> {
    // dbg!(tokio_serial::available_ports());

    // Create a Serial connection, and split it into two halves.
    let port = tokio_serial::new(path, baud).open_native_async()?;
    let (sink, stream) = PacketCodec.framed(port).split();

    // Ignore invalid packets.
    let stream = stream.filter_map(|result| future::ready(result.ok()));

    // Create a new link.
    Ok(Link::new(sink, stream, GCS_SYSTEM_ID, GCS_COMPONENT_ID))
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
