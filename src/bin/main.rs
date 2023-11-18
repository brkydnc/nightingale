use mavlink::{MessageData, MavlinkVersion, ardupilotmega::MavMessage as ArdupilotMessage};
use nightingale::{TcpConnection, error::Error as NightingaleError};
use tokio;
// use tokio::io::AsyncReadExt;
// use tokio::net::TcpStream;

async fn send_message(connection: &mut TcpConnection) {
    use mavlink::ardupilotmega::*;

    let target_system = 1;
    let target_component = 0;

    let data = COMMAND_INT_DATA {
        command: MavCmd::MAV_CMD_SET_MESSAGE_INTERVAL,
        param1: GLOBAL_POSITION_INT_DATA::ID as f32,
        param2: 1_000_000 as f32,
        target_system,
        target_component,
        ..Default::default()
    };

    let command = MavMessage::COMMAND_INT(data);

    match connection.send(target_system, target_component, &command).await {
        Ok(n) => {
            eprintln!("Command sent, {n} bytes were written.");
            dbg!(command);
        }
        Err(err)  => { eprintln!("Error sending command: {:?}", err); }
    }
}

#[tokio::main]
async fn main() -> Result<(), NightingaleError> {
    let mut connection = TcpConnection::connect("127.0.0.1:5762", MavlinkVersion::V2)
        .await
        .expect("Error establishing connection");

    send_message(&mut connection).await;

    let mut counter = 0;

    loop {
        match connection.receive::<ArdupilotMessage>().await {
            Ok(message) => {
                use ArdupilotMessage::*;

                match message {
                    HEARTBEAT(_) | TIMESYNC(_) => { },
                    _ => { eprintln!("order = {:?}, {:#?}", counter, message); }
                }
            }

            Err(err)  => {
                dbg!(err);
            }
        }

        counter += 1;
    }
}
