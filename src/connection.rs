use crate::error::Result;
use crate::message;
use async_trait::async_trait;

use mavlink::{ Message, MavHeader };
use tokio::net::{
    tcp::{OwnedReadHalf, OwnedWriteHalf},
    TcpStream, ToSocketAddrs,
};

#[async_trait]
pub trait Sender {
    async fn send<M: Message + Sync>(&mut self, system_id: u8, component_id: u8, message: &M) -> Result<usize>;
}

#[async_trait]
pub trait Receiver: Send {
    async fn receive<M: Message>(&mut self) -> Result<M>;
}

#[async_trait]
pub trait Connection {
    type Sender: Sender;
    type Receiver: Receiver;
    async fn connect<A: ToSocketAddrs + Send>(addr: A) -> Result<(Self::Sender, Self::Receiver)>;
}

pub struct TcpReceiver(OwnedReadHalf);

#[async_trait]
impl Receiver for TcpReceiver {
    async fn receive<M: Message>(&mut self) -> Result<M> {
        message::v2::read(&mut self.0).await
    }
}

pub struct TcpSender {
    writer: OwnedWriteHalf,
    sequence: u8,
}

#[async_trait]
impl Sender for TcpSender {
    async fn send<M>(&mut self, system_id: u8, component_id: u8, message: &M) -> Result<usize>
        where M: Message + Sync
    {
        let header = MavHeader {
            system_id,
            component_id,
            sequence: self.sequence,
        };

        self.sequence += 1;

        message::v2::write(&mut self.writer, header, message).await
    }
}

pub struct TcpConnection;

#[async_trait]
impl Connection for TcpConnection {
    type Sender = TcpSender;
    type Receiver = TcpReceiver;

    async fn connect<A>(addr: A) -> Result<(Self::Sender, Self::Receiver)>
        where A: ToSocketAddrs + Send
    {
        let stream = TcpStream::connect(addr).await?;
        let (reader, writer) = stream.into_split();
        let sender = TcpSender { writer, sequence: 0 };
        let receiver = TcpReceiver(reader);

        Ok((sender, receiver))
    }
}
