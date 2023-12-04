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

use tokio_util::codec::{FramedRead, FramedWrite};
use futures::stream::StreamExt;
use mavlink::Message as MessageExt;
use nightingale::{
    link::Link,
    wire::{Message, PacketDecoder, PacketEncoder}
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connection = tokio::net::TcpStream::connect("127.0.0.1:5762").await?;
    let (reader, writer) = connection.into_split();

    let sink = FramedWrite::new(writer, PacketEncoder);
    let stream = FramedRead::new(reader, PacketDecoder);

    let link = Link::new(sink, stream);

    let mut subscriber = link.subscribe();

    while let Ok(r) = subscriber.changed().await {
        dbg!(r);
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    Ok(())
}
