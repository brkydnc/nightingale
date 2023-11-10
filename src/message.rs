use crate::error::Result;
use mavlink::{MavlinkVersion, Message};
use std::ops::{Deref, DerefMut};
use tokio::io::AsyncReadExt;

pub mod v2 {
    use super::*;

    use mavlink::{MAVLinkV2MessageRaw as Raw, MAV_STX_V2 as MAGIC};

    const MAVLINK_IFLAG_SIGNED: u8 = 0x01;

    #[repr(transparent)]
    pub struct RawMessage(Raw);

    impl RawMessage {
        const SIZE: usize = std::mem::size_of::<Raw>();
        const HEADER_SIZE: usize = 9;

        pub fn parse<M: Message>(&self) -> Result<M> {
            M::parse(MavlinkVersion::V2, self.message_id(), self.payload()).map_err(From::from)
        }

        pub async fn read<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Self> {
            // TODO: Maybe use BufReader here?
            while reader.read_u8().await? != MAGIC {}

            // Initialize raw message buffer.
            let mut buffer = [0; Self::SIZE];

            // Write STX, read header.
            buffer[0] = MAGIC;
            reader
                .read_exact(&mut buffer[1..=Self::HEADER_SIZE])
                .await?;

            // Extract payload length from header
            let len = buffer[1] as usize;

            // Determine signature size using incompatibility flags.
            let sig = 13 * (buffer[2] & MAVLINK_IFLAG_SIGNED) as usize;

            // Read payload + checksum + signature.
            reader.read_exact(&mut buffer[10..(12 + len + sig)]).await?;

            // XXX: The current implementation of MAVLinkV2MessageRaw does not
            // allow you to access its internal buffer. This is a hack-around
            // to get things done *until* the `mavlink` crate offers a new way
            // to use its API in a flexible way. Hence, this is a *highly*
            // unsafe transmute, as MAVLinkV2MessageRaw does not derive
            // `repr(transparent)` and things can go wrong once the type
            // definition of MAVLinkV2MessageRaw changes.
            //
            // TODO: Use a safer alternative. Or remove this transmute
            // completely if `mavlink` crate offers a way to access its
            // internal buffer.
            let raw = unsafe { std::mem::transmute(buffer) };

            Ok(Self(raw))
        }
    }

    impl Deref for RawMessage {
        type Target = Raw;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for RawMessage {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    pub async fn read<R, M>(reader: &mut R) -> Result<M>
    where
        R: AsyncReadExt + Unpin,
        M: Message,
    {
        // Loop until we receive a valid message, in terms of CRC.
        loop {
            let raw = RawMessage::read(reader).await?;
            if raw.has_valid_crc::<M>() {
                break raw.parse();
            }
        }
    }
}

pub mod v1 {
    use super::*;

    use mavlink::{MAVLinkV1MessageRaw as Raw, MAV_STX as MAGIC};

    #[repr(transparent)]
    pub struct RawMessage(Raw);

    impl RawMessage {
        const SIZE: usize = std::mem::size_of::<Raw>();
        const HEADER_SIZE: usize = 5;

        pub fn parse<M: Message>(&self) -> Result<M> {
            M::parse(MavlinkVersion::V1, self.message_id() as u32, self.payload())
                .map_err(From::from)
        }

        pub async fn read<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Self> {
            // TODO: Maybe use BufReader here?
            while reader.read_u8().await? != MAGIC {}

            // Initialize raw message buffer.
            let mut buffer = [0; Self::SIZE];

            // Write STX, read header.
            buffer[0] = MAGIC;
            reader
                .read_exact(&mut buffer[1..=Self::HEADER_SIZE])
                .await?;

            // Extract payload length from header
            let len = buffer[1] as usize;

            // Read payload + checksum + signature.
            reader.read_exact(&mut buffer[6..(8 + len)]).await?;

            // XXX: The current implementation of MAVLinkV1MessageRaw does not
            // allow you to access its internal buffer. This is a hack-around
            // to get things done *until* the `mavlink` crate offers a new way
            // to use its API in a flexible way. Hence, this is a *highly*
            // unsafe transmute, as MAVLinkV1MessageRaw does not derive
            // `repr(transparent)` and things can go wrong once the type
            // definition of MAVLinkV1MessageRaw changes.
            //
            // TODO: Use a safer alternative. Or remove this transmute
            // completely if `mavlink` crate offers a way to access its
            // internal buffer.
            let raw = unsafe { std::mem::transmute(buffer) };

            Ok(Self(raw))
        }
    }

    impl Deref for RawMessage {
        type Target = Raw;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for RawMessage {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    pub async fn read<R, M>(reader: &mut R) -> Result<M>
    where
        R: AsyncReadExt + Unpin,
        M: Message,
    {
        // Loop until we receive a valid message, in terms of CRC.
        loop {
            let raw = RawMessage::read(reader).await?;
            if raw.has_valid_crc::<M>() {
                break raw.parse();
            }
        }
    }
}
