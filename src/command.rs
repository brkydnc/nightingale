use crate::{
    link::{MessageHandler, MessageID},
    prelude::*,
};

use mavlink::{ Message, MavlinkVersion, error::ParserError };

use std::{
    time::Duration,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::{
    time::timeout,
    sync::{ mpsc, oneshot }
};

use bytes::{ BytesMut, BufMut };

const COMMAND_RETRY: usize = 3;
const COMMAND_ACK_ID: MessageID = 77;
const COMMAND_ACK_TIMEOUT: Duration = Duration::from_millis(1_000);

type AckSenderMap = HashMap<CommandID, mpsc::Sender<CommandAck>>;

pub struct CommandProtocol<M, C: Connection> {
    link: Arc<Link<M, C>>,
    handler: MessageHandler<M>,
    map: Arc<Mutex<AckSenderMap>>,
}

impl<M: Message + Sync + 'static, C: Connection + 'static> CommandProtocol<M, C> {
    pub fn new(link: Arc<Link<M, C>>) -> Self {
        let map = Default::default();
        let handler = Self::ack_handler(Arc::clone(&map));

        // Register the COMMAND_ACK handler.
        link.register(COMMAND_ACK_ID, handler.clone());

        Self { link, handler, map }
    }


    // TODO: We assume cmd.target_system == header.target_system, is this correct?
    async fn command_int(
        cmd: CommandInt,
        link: Arc<Link<M, C>>,
        map: Arc<Mutex<AckSenderMap>>,
        mut rx: mpsc::Receiver<CommandAck>,
        tx: oneshot::Sender<Option<()>>,
    ) {
        // Create the message we for the command to be able to send it.
        let msg = cmd.to_message::<M>()
            .expect("COMMAND_INT should be valid in the provided dialect");

        // Send the message, this is our first attempt.
        // TODO: What should we do if `send` fails? (Currently it is ignored)
        let _ = link.send(cmd.target_system, cmd.target_component, &msg).await;

        // The number of attempts to receive an ack.
        let mut attempts = 1;

        // TODO: Maybe use a result instead of option for the return value?
        let ret = loop {
            match timeout(COMMAND_ACK_TIMEOUT, rx.recv()).await {
                // We have received an ack.
                Ok(received) => break received.map(|_| ()),

                // Timeout for the attempt.
                Err(_elapsed) => {
                    // Every attempt timed out. We received no ack, just stop.
                    if attempts >= COMMAND_RETRY { break None; } 

                    // Retry sending the message.
                    let _ = link.send(cmd.target_system, cmd.target_component, &msg).await;
                    attempts += 1;
                }
            }
        };

        // Send the return value. Don't care if it is delivered correctly.
        let _ = tx.send(ret);


        // Remove COMMAND_ACK sender from the map.
        map
            .lock()
            .expect("Command protocol's map is poisoned.")
            .remove(&cmd.command);
    }

    fn ack_handler(map: Arc<Mutex<AckSenderMap>>) -> MessageHandler<M> {
        Arc::new(move |message: &M| {
            // TODO: Conversion *hack* goes here.
            let ack = CommandAck {
                command: 0,
                result: 0,
                progress: 0,
                result_param2: 0,
                target_system: 0,
                target_component: 0,
            };

            let guard = map.lock().expect("Command protocol's map is poisoned.");

            if let Some(sender) = guard.get(&ack.command) {
                let sender = sender.clone();
                drop(guard);

                // TODO: Maybe this is an overkill?
                tokio::spawn(async move {
                    sender.send(ack);
                });
            }
        })
    }
}

impl<M: Message + Sync + 'static, C: Connection + 'static> CommandProtocol<M, C> {
    fn drop(&mut self) {
        self.link.unregister(COMMAND_ACK_ID, self.handler.clone());
    }
}

type CommandID = u16;

pub struct CommandAck {
    pub command: CommandID,
    pub result: u8,
    pub progress: u8,
    pub result_param2: i32,
    pub target_system: u8,
    pub target_component: u8,
}

pub struct CommandInt {
    pub param1: f32,
    pub param2: f32,
    pub param3: f32,
    pub param4: f32,
    pub x: i32,
    pub y: i32,
    pub z: f32,
    pub command: CommandID,
    pub target_system: u8,
    pub target_component: u8,
    pub frame: u8,
    pub current: u8,
    pub autocontinue: u8,
}

impl CommandInt {
    /// The id of COMMAND_INT.
    const ID: u32 = 75;

    /// Create a `mavlink::Message` message from `CommandInt`.
    ///
    /// Currently, the `mavlink` crate does not provide a way to represent 
    /// commands in a common way for all dialects. To get around this, we
    /// serialize `CommandInt`, and then deserialize into a message type.
    fn to_message<M: Message>(&self) -> std::result::Result<M, ParserError> {
        let mut bytes = BytesMut::new();
        bytes.put_f32_le(self.param1);
        bytes.put_f32_le(self.param2);
        bytes.put_f32_le(self.param3);
        bytes.put_f32_le(self.param4);
        bytes.put_i32_le(self.x);
        bytes.put_i32_le(self.y);
        bytes.put_f32_le(self.z);
        bytes.put_u16_le(self.command as u16);
        bytes.put_u8(self.target_system);
        bytes.put_u8(self.target_component);
        bytes.put_u8(self.frame as u8);
        bytes.put_u8(self.current);
        bytes.put_u8(self.autocontinue);

        // According to the Mavlink protocol, there should be at least one byte
        // in the payload, even if it is zero, so skip 1.
        let trailing_zero_bytes = bytes[1..]
            .iter()
            .rev()
            .filter(|&b| *b == 0).count();

        // Calculate the true payload length.
        let payload_length = bytes.len() - trailing_zero_bytes;

        // Parse the command into message. 
        M::parse(MavlinkVersion::V2, Self::ID, &bytes[..payload_length])
    }
}

pub struct CommandLong {
    pub param1: f32,
    pub param2: f32,
    pub param3: f32,
    pub param4: f32,
    pub param5: f32,
    pub param6: f32,
    pub param7: f32,
    pub command: CommandID,
    pub target_system: u8,
    pub target_component: u8,
    pub confirmation: u8,
}
