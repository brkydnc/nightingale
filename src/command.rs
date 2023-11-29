use crate::{
    link::{MessageHandler, MessageID},
    prelude::*,
};

use mavlink::Message;

use std::{
    collections::HashMap,
    sync::mpsc::{channel, Receiver, Sender},
    sync::{Arc, Mutex},
};

const COMMAND_ACK_ID: MessageID = 77;

type AckSenderMap = HashMap<CommandID, Sender<CommandAck>>;

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

//
// async fn command_int_task(command: _, link: , receiver: Receiver<CommandAck>, callback) {
//      const COMMAND_ACK_TIMEOUT: usize = 1_000;
//
//      // XXX: Watchout for wrap-arounds on usize, or the task will
//      attempt to send the command usize::MAX times.
//      const COMMAND_RETRY: usize = 3;
//
//      let mut retry = COMMAND_RETRY;
//
//      link.send(command).await;
//
//      loop {
//          select! {
//              result  = receiver.recv().await => {
//                  callback(Ok(result));
//              }
//              _ = tokio::time::sleep(Duration::from_millis(COMMAND_ACK_TIMEOUT)).await {
//                  if retry == 1 {
//                      break;
//                  } else {
//                      retry -= 1;
//                      link.send(command).await;
//                  }
//              }
//          }
//      }
//
//      callback(Err());
// }
//

type CommandID = u16;

pub struct CommandAck {
    pub command: CommandID,
    pub result: u8,
    pub progress: u8,
    pub result_param2: i32,
    pub target_system: u8,
    pub target_component: u8,
}

pub struct CommmandInt {
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
