use crate::{
    wire::Packet,
    link::{Subscriber, Link},
    dialect::{
        Message,
        COMMAND_INT_DATA as CommandInt
    }
};

use futures::Sink;
use std::sync::Arc;

pub struct CommandProtocol<T> {
    link: Arc<Link<T>>,
    incoming: Subscriber,
    system: u8,
    component: u8,
}

impl<T: Sink<Packet> + Unpin + 'static> CommandProtocol<T> {
    pub fn new(system: u8, component: u8, link: Arc<Link<T>>) -> Self {
        let incoming = link.subscribe();
        Self { link, incoming, system, component }
    }

    async fn send_command_int(&mut self, system_id: u8, component_id: u8, cmd: CommandInt) {
        let message = Message::COMMAND_INT(cmd.clone());

        self.link.send(self.system, self.component, message).await;

        let mut attempts = 1;
        // TODO: Maybe use a result instead of option for the return value?
        let ret = loop {
            match timeout(COMMAND_ACK_TIMEOUT, rx.recv()).await {
                // We have received an ack.
                Ok(received) => break received.map(|_| ()),

                // Timeout for the attempt.
                Err(_elapsed) => {
                    // Every attempt timed out. We received no ack, just stop.
                    if attempts >= COMMAND_RETRY {
                        break None;
                    }

                    // Retry sending the message.
                    let _ = link
                        .send(cmd.target_system, cmd.target_component, &msg)
                        .await;
                    attempts += 1;
                }
            }
        };

        // Send the return value. Don't care if it is delivered correctly.
        let _ = tx.send(ret);

        // Remove COMMAND_ACK sender from the map.
        map.lock()
            .expect("Command protocol's map is poisoned.")
            .remove(&(cmd.command as CommandID));
    }
}
