use nightingale::{
    dialect::{
        MavResult::MAV_RESULT_ACCEPTED as Accepted,
        *
    },
    link::Link,
    error::Error,
    wire::{Packet, PacketCodec},
    component::Component,
};
use std::{net::SocketAddr, time::Duration, io::Error as IoError, future::Future};
use tokio::net::UdpSocket;
use tokio_util::udp::UdpFramed;
use futures_util::{future, SinkExt, StreamExt};
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

    eprintln!("Setting AUTOPILOT_VERSION message rate...");
    match autopilot.set_message_interval(AUTOPILOT_VERSION_DATA::ID, Duration::from_secs(1)).await? {
        Accepted => eprintln!("AUTOPILOT_VERSION message interval set."),
        e => panic!("Couldn't set AUTOPILOT_VERSION message interval [{e:?}], aborting..."),
    }

    let _ = tasks.await;

    Ok(())
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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
            Message::STATUSTEXT(STATUSTEXT_DATA { severity, text, .. }) => {
                let content = std::str::from_utf8(text).expect("a valid utf8 string");
                eprintln!("[STATUS_TEXT] ({severity:?}) {content}");
            },
            Message::AUTOPILOT_VERSION(data) => {
                eprintln!("{:#?}", data);
            },
            _ => {},
        }
    }).await;
}
