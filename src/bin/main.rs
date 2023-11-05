use mavlink::MavlinkVersion;
use nightingale::TcpConnection;
use tokio::net::TcpStream;
use tokio::io::AsyncReadExt;
use tokio;

#[tokio::main]
async fn main() {
    ng().await;
}

async fn mav() {
    let mut connection = mavlink::connect::<mavlink::common::MavMessage>("tcpout:127.0.0.1:5762").expect("Error establishing connection.");

    loop {
        match connection.recv() {
            Ok((_, msg)) => { dbg!(msg); },
            _ => { }

        }
    }
}

async fn ng() {
    let mut connection = TcpConnection::connect("127.0.0.1:5762", MavlinkVersion::V2)
        .await
        .expect("Error establishing connection");

    loop {
        match connection.recv::<mavlink::ardupilotmega::MavMessage>().await {
            Ok(message) => { dbg!(message); },
            _ => { eprintln!("error") },
        }
    }
}

async fn tk() {
    let mut stream = TcpStream::connect("127.0.0.1:5762").await.expect("err");
    // let mut reader = tokio::io::BufReader::new(stream);
    
    let mut bytes = [0u8; 1024];

    loop {
        match stream.read(&mut bytes).await {
            Ok(n) => {
                assert_eq!(bytes[0], mavlink::MAV_STX_V2);
                dbg!(n);
            },
            _ => { }
        }
    }
}
