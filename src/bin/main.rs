use mavlink::MavlinkVersion;
use nightingale::TcpConnection;
use tokio;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    tk().await;
}

async fn mav() {
    let mut connection = mavlink::connect::<mavlink::common::MavMessage>("tcpout:127.0.0.1:5762")
        .expect("Error establishing connection.");

        match connection.recv() {
            Ok((_, msg)) => {
                dbg!(msg);
            }
            _ => {}
        }
}

async fn ng() {
    let mut connection = TcpConnection::connect("127.0.0.1:5762", MavlinkVersion::V2)
        .await
        .expect("Error establishing connection");

    loop {
        match connection.receive::<mavlink::ardupilotmega::MavMessage>().await {
            Ok(message) => { dbg!(message); },
            Err(err) => { dbg!(err); },
        }
    }
}

async fn tk() {
    let mut stream = TcpStream::connect("127.0.0.1:5762").await.expect("err");
    // let mut reader = tokio::io::BufReader::new(stream);

    let mut bytes = [0u8; 1024];

        match stream.read(&mut bytes).await {
            Ok(n) => {
                assert_eq!(bytes[0], mavlink::MAV_STX_V2);
                dbg!(n);
            }
            _ => {}
        }
}
