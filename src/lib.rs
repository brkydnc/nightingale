use tokio::{
    self,
    io::AsyncReadExt,
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream, ToSocketAddrs,
    },
};

use mavlink::{self, MavlinkVersion, Message, MAV_STX_V2};

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    MavlinkParserError(mavlink::error::ParserError),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<mavlink::error::ParserError> for Error {
    fn from(err: mavlink::error::ParserError) -> Self {
        Error::MavlinkParserError(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct TcpConnection {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    protocol: MavlinkVersion,
}

impl TcpConnection {
    pub async fn connect(addr: impl ToSocketAddrs, protocol: MavlinkVersion) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        let (reader, writer) = stream.into_split();
        let connection = Self {
            reader,
            writer,
            protocol,
        };

        Ok(connection)
    }

    pub async fn recv<M: Message>(&mut self) -> Result<M> {
        let packet = MavlinkPacketV2::using::<M>(&mut self.reader).await?;
        packet.parse()
    }
}

struct MavlinkPacketV2 {
    buf: [u8; Self::SIZE],
}

impl MavlinkPacketV2 {
    const SIZE: usize = 280;
    const MAGIC: u8 = MAV_STX_V2;

    fn message_id(&self) -> u32 {
        u32::from_le_bytes([self.buf[7], self.buf[8], self.buf[9], 0])
    }

    fn payload(&self) -> &[u8] {
        let len = self.buf[1] as usize;
        &self.buf[10..=(9 + len)]
    }

    fn parse<M: Message>(&self) -> Result<M> {
        M::parse(MavlinkVersion::V2, self.message_id(), self.payload()).map_err(Into::into)
    }

    // TODO: Loop until we receive a VALID (in terms of CRC) packet.
    // loop { if !has_valid_crc() { continue; } }
    async fn using<M: Message>(reader: &mut (impl AsyncReadExt + Unpin)) -> Result<Self> {
        while reader.read_u8().await? != MavlinkPacketV2::MAGIC {}

        let mut buf = [Self::MAGIC; Self::SIZE];
        reader.read_exact(&mut buf[1..]).await?;

        Ok(Self { buf })
    }
}
