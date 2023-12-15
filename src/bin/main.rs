use std::{ sync::Arc, time::Duration };
use tokio::net::tcp::OwnedWriteHalf;
use tokio_util::codec::{FramedRead, FramedWrite};
use nightingale::{
    link::{Link, Subscriber},
    wire::{PacketDecoder, PacketEncoder},
    dialect::{
        Message,
        MavType,
        MavAutopilot,
        MavState,
        MavModeFlag,
        MavCmd,
        MessageExt,
        HEARTBEAT_DATA,
        COMMAND_INT_DATA,
    }
};

const GCS_SYSTEM_ID: u8 = 255;
const GCS_COMPONENT_ID: u8 = 1;

type TcpLink = Link<FramedWrite<OwnedWriteHalf, PacketEncoder>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connection = tokio::net::TcpStream::connect("127.0.0.1:5762").await?;
    let (reader, writer) = connection.into_split();

    let sink = FramedWrite::new(writer, PacketEncoder);
    let stream = FramedRead::new(reader, PacketDecoder);

    let link = Arc::new(Link::new(sink, stream));

    let receive = tokio::spawn(receive_heartbeat(link.subscribe()));
    let broadcast = tokio::spawn(broadcast_heartbeat(link.clone()));
    let gps = tokio::spawn(receive_global_position_int(link.subscribe()));

    // Receive GLOBAL_POSITION_INT every  seconds
    set_message_interval(link.clone(), 33, Duration::from_secs(1)).await;

    let _ = tokio::join!(receive, broadcast, gps);

    Ok(())
}

async fn receive_global_position_int(mut subscriber: Subscriber) {
    while let Ok(p) = subscriber.wait_for(|p| p.message.message_id() == 33).await {
        eprintln!("{:#?}", p);
    }
}

async fn receive_heartbeat(mut subscriber: Subscriber) {
    while let Ok(p) = subscriber.wait_for_message(0).await {
        eprintln!("HEARTBEAT sysid: {:?}, cmpid: {:?}", p.header.system_id, p.header.component_id);
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

        link.send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, heartbeat).await;
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

    link.send(GCS_SYSTEM_ID, GCS_COMPONENT_ID, command).await;
}
