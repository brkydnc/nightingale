mod error;
mod message;

use error::Result;

use tokio::{
    self,
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream, ToSocketAddrs,
    },
};

use mavlink::{self, MavlinkVersion, Message};

pub struct TcpConnection {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    protocol: MavlinkVersion,
}

impl TcpConnection {
    pub async fn connect(addr: impl ToSocketAddrs, protocol: MavlinkVersion) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        let (reader, writer) = stream.into_split();
        Ok(Self {
            reader,
            writer,
            protocol,
        })
    }

    pub async fn receive<M: Message>(&mut self) -> Result<M> {
        use MavlinkVersion::*;

        match self.protocol {
            V1 => message::v1::read(&mut self.reader).await,
            V2 => message::v2::read(&mut self.reader).await,
        }
    }
}
