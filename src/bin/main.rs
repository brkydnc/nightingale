// use mavlink::{
//     ardupilotmega::{MavMessage as Message, *},
//     MessageData,
// };
// use nightingale::{connection::TcpConnection as Tcp, error::Error as NightingaleError, link::Link};
// use std::sync::Arc;
// use tokio;
// // use tokio::io::AsyncReadExt;
// // use tokio::net::TcpStream;

// #[tokio::main]
// async fn main() -> Result<(), NightingaleError> {
//     let handler = Arc::new(|message: &Message| {
//         dbg!(message);
//     });

//     let link: Link<Tcp> = Link::connect("192.168.1.105:5763").await?;

//     link.register(GLOBAL_POSITION_INT_DATA::ID, handler.clone());

//     let command = Message::COMMAND_INT(COMMAND_INT_DATA {
//         command: MavCmd::MAV_CMD_SET_MESSAGE_INTERVAL,
//         param1: GLOBAL_POSITION_INT_DATA::ID as f32,
//         param2: 1_000_000 as f32,
//         target_system: 1,
//         target_component: 0,
//         ..Default::default()
//     });

//     link.send(1, 0, &command).await?;

//     // std::thread::sleep(std::time::Duration::from_secs(2));

//     // link.unregister(GLOBAL_POSITION_INT_DATA::ID, handler);

//     loop {}
// }
//
fn main() {

}
