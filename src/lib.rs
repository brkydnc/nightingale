mod error;

use crate::error::Result;

use tokio::{
    self,
    io::AsyncReadExt,
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream, ToSocketAddrs,
    },
};

use mavlink::{self, MavlinkVersion, Message, MAV_STX_V2};

pub struct TcpConnection {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    protocol: MavlinkVersion,
}

impl TcpConnection {
    pub async fn connect(addr: impl ToSocketAddrs, protocol: MavlinkVersion) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        let (reader, writer) = stream.into_split();
        Ok(Self { reader, writer, protocol })
    }

    pub async fn receive<M: Message>(&mut self) -> Result<M> {
        use MavlinkVersion::*;

        match self.protocol {
            V1 => unimplemented!(),
            V2 => RawPacketV2::read(&mut self.reader).await?.parse()
        }
    }
}

struct RawPacketV2 {
    buf: [u8; Self::SIZE],
}

impl RawPacketV2 {
    const MAGIC: u8 = MAV_STX_V2;
    const SIZE: usize = 280;

    fn new() -> Self {
        let mut buf = [0; Self::SIZE];
        buf[0] = Self::MAGIC;
        Self { buf }
    }

    fn tail(&mut self) -> &mut [u8] {
        &mut self.buf[1..]
    }

    fn message_id(&self) -> u32 {
        u32::from_le_bytes([self.buf[7], self.buf[8], self.buf[9], 0])
    }

    fn payload(&self) -> &[u8] {
        let len = self.buf[1] as usize;
        &self.buf[10..=(9 + len)]
    }

    fn parse<M: Message>(&self) -> Result<M> {
        M::parse(
            MavlinkVersion::V2,
            self.message_id(),
            self.payload()
        ).map_err(From::from)
    }

    // TODO: Loop until we receive a VALID (in terms of CRC) packet.
    async fn read<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Self> {
        // TODO: Maybe use BufReader here?
        while reader.read_u8().await? != Self::MAGIC { }

        // XXX: There may be more than one messages in a single TCP packet.
        // Currently, the implementation below effectively *ignores* the
        // messages that come after the first message. This is because we
        // `.read()` into a 280 byte buffer, and only interact with the first
        // message.
        let mut packet = Self::new();
        reader.read(packet.tail()).await?;

        Ok(packet)
    }
}
