pub mod error;
pub mod message;

use error::Result;

use tokio::{
    self,
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream, ToSocketAddrs,
    },
};

use mavlink::{self, MavlinkVersion::{ self, V1, V2 }, Message, MavHeader};

pub struct TcpConnection {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    protocol: MavlinkVersion,
    sequence: u8,
}

impl TcpConnection {
    pub async fn connect(addr: impl ToSocketAddrs, protocol: MavlinkVersion) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        let (reader, writer) = stream.into_split();
        Ok(Self {
            reader,
            writer,
            protocol,
            sequence: 0,
        })
    }

    pub async fn receive<M: Message>(&mut self) -> Result<M> {
        match self.protocol {
            V1 => message::v1::read(&mut self.reader).await,
            V2 => message::v2::read(&mut self.reader).await,
        }
    }

    pub async fn send<M: Message>(&mut self, system_id: u8, component_id: u8, message: &M) -> Result<usize> {
        let header = MavHeader {
            system_id,
            component_id,
            sequence: self.sequence
        };

        self.sequence += 1;

        match self.protocol {
            V1 => message::v1::write(&mut self.writer, header, message).await,
            V2 => message::v2::write(&mut self.writer, header, message).await,
        }
    }

    pub fn set_protocol(&mut self, protocol: MavlinkVersion) {
        self.protocol = protocol;
    }
}
