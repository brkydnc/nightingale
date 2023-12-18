use crate::dialect::{Header, Message, MessageExt};
use std::io::{Error, ErrorKind::InvalidData};

use mavlink::{MAVLinkV2MessageRaw as RawPacket, MavlinkVersion::V2, MAV_STX_V2 as MAGIC_BYTE};

use tokio_util::{
    bytes::{Buf, BytesMut},
    codec::{Decoder, Encoder},
};

use crc16::{State, MCRF4XX};

#[derive(Debug, Clone)]
pub struct Packet {
    pub header: Header,
    pub message: Message,
}

impl Packet {
    const HEADER_SIZE: usize = 9;
    const CKSUM_SIZE: usize = 2;
    const IFLAG_SIGNED: u8 = 0x01;
}

impl Default for Packet {
    fn default() -> Self {
        let header = Header {
            system_id: 255,
            component_id: 0,
            sequence: 0,
        };
        let message = Message::HEARTBEAT(Default::default());
        Self { header, message }
    }
}

pub struct PacketCodec;

impl Encoder<Packet> for PacketCodec {
    type Error = Error;

    fn encode(
        &mut self,
        packet: Packet,
        dst: &mut BytesMut,
    ) -> std::result::Result<(), Self::Error> {
        let mut raw = RawPacket::new();
        raw.serialize_message(packet.header, &packet.message);
        dst.extend_from_slice(raw.raw_bytes());
        Ok(())
    }
}

impl Decoder for PacketCodec {
    type Item = Packet;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // TODO: Do not search for the magic byte every time this function gets
        // called. Store this information in a boolean, and check that boolean
        // until we advance the buffer. After we advance, clear the boolean.
        //
        // Find the position of the magic byte.
        let magic_byte_position = src
            .iter()
            .position(|n| *n == MAGIC_BYTE)
            .unwrap_or(src.len());

        // Advance the buf, so that the first byte is *always* the magic byte.
        src.advance(magic_byte_position);

        // Ensure that we have *at least* the header of the packet.
        if src.len() < Packet::HEADER_SIZE {
            return Ok(None);
        }

        // Extract payload size from header.
        let payload_size = src[1] as usize;

        // Determine signature size using incompatibility flags.
        let signature_size = 13 * (src[2] & Packet::IFLAG_SIGNED) as usize;

        // Calculate the total packet size.
        let packet_size =
            1 + Packet::HEADER_SIZE + payload_size + Packet::CKSUM_SIZE + signature_size;

        // Ensure that we have the required amount of bytes to read packet.
        if src.len() < packet_size {
            return Ok(None);
        }

        // Calculate CRC and validate the packet.
        let message_id = u32::from_le_bytes([src[7], src[8], src[9], 0]);
        let crc_extra = Message::extra_crc(message_id);
        let crc_data = &src[1..(1 + Packet::HEADER_SIZE + payload_size)];

        let mut state = State::<MCRF4XX>::new();
        state.update(crc_data);
        state.update(&[crc_extra]);

        let crc = state.get();
        let offset = 1 + Packet::HEADER_SIZE + payload_size;
        let checksum = u16::from_le_bytes([src[offset], src[offset + 1]]);

        // Validate CRC.
        if crc == checksum {
            let payload_begin = 1 + Packet::HEADER_SIZE;
            let payload_end = payload_begin + payload_size;
            let payload = &src[payload_begin..payload_end];

            let message = Message::parse(V2, message_id, payload)
                .map_err(|_| Error::new(InvalidData, "Invalid message."))?;

            let packet = Packet {
                message,
                header: Header {
                    sequence: src[4],
                    system_id: src[5],
                    component_id: src[6],
                },
            };

            // Clear the current packet.
            src.advance(packet_size);

            // Return valid packet.
            return Ok(Some(packet));
        } else {
            // Clear the current packet.
            src.advance(packet_size);

            // Return invalid CRC error.
            return Err(Error::new(InvalidData, "Invalid CRC."));
        }
    }
}
