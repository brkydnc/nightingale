use crate::{
    link::{MessageHandler, MessageID},
    prelude::*,
};

use mavlink::{
    ardupilotmega as apm,
    ardupilotmega::MavMessage as Message,
};

use std::{
    time::Duration,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::{
    time::timeout,
    sync::{ mpsc, oneshot }
};

type CommandID = u16;
type CommandInt = apm::COMMAND_INT_DATA;
type CommandLong = apm::COMMAND_LONG_DATA;
type CommandAck = apm::COMMAND_ACK_DATA;

const COMMAND_RETRY: usize = 3;
const COMMAND_ACK_ID: MessageID = 77;
const COMMAND_ACK_TIMEOUT: Duration = Duration::from_millis(1_000);

type AckSenderMap = HashMap<CommandID, mpsc::Sender<CommandAck>>;

pub struct CommandProtocol<C: Connection> {
    link: Arc<Link<C>>,
    handler: MessageHandler,
    map: Arc<Mutex<AckSenderMap>>,
}

impl<C: Connection + 'static> CommandProtocol<C> {
    pub fn new(link: Arc<Link<C>>) -> Self {
        let map = Default::default();
        let handler = Self::ack_handler(Arc::clone(&map));

        // Register the COMMAND_ACK handler.
        link.register(COMMAND_ACK_ID, handler.clone());

        Self { link, handler, map }
    }


    // TODO: We assume cmd.target_system == header.target_system, is this correct?
    async fn command_int(
        cmd: CommandInt,
        link: Arc<Link<C>>,
        map: Arc<Mutex<AckSenderMap>>,
        mut rx: mpsc::Receiver<CommandAck>,
        tx: oneshot::Sender<Option<()>>,
    ) {
        // Create the message we for the command to be able to send it.
        let msg = Message::COMMAND_INT(cmd.clone());

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
            .remove(&(cmd.command as CommandID));
    }

    fn ack_handler(map: Arc<Mutex<AckSenderMap>>) -> MessageHandler {
        Arc::new(move |message: &Message| {
            // TODO: Conversion *hack* goes here.
            let Message::COMMAND_ACK(ack) = message else { unreachable!() };

            let guard = map.lock().expect("Command protocol's map is poisoned.");

            if let Some(sender) = guard.get(&(ack.command as CommandID)) {
                let sender = sender.clone();
                let ack = ack.clone();
                drop(guard);

                // TODO: Maybe this is an overkill?
                tokio::spawn(async move {
                    let _ = sender.send(ack).await;
                });
            }
        })
    }
}

impl<C: Connection + 'static> CommandProtocol<C> {
    fn drop(&mut self) {
        self.link.unregister(COMMAND_ACK_ID, self.handler.clone());
    }
}
