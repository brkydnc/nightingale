use crate::{
    dialect::{Header, MavCmd, Message, MessageId},
    wire::Packet,
};

use futures::prelude::*;
use mavlink::Message as MessageTrait;
use std::ops::{Deref, DerefMut};
use tokio::sync::{
    watch::{self, error::RecvError},
    Mutex,
};

pub struct Link<T> {
    sink: Mutex<SequencedSink<T>>,
    subscriber: Subscriber,
}

impl<T: Sink<Packet> + Unpin + 'static> Link<T> {
    pub fn new<U>(sink: T, stream: U) -> Self
    where
        U: Stream<Item = DecoderResult> + Unpin + Send + 'static,
    {
        let (publisher, subscriber) = watch::channel(Default::default());
        let subscriber = Subscriber(subscriber);
        let sink = SequencedSink { sink, sequence: 0 }.into();
        tokio::spawn(Self::receiver(stream, publisher));
        Self { sink, subscriber }
    }

    pub fn subscribe(&self) -> Subscriber {
        self.subscriber.clone()
    }

    pub async fn send(&self, system_id: u8, component_id: u8, message: Message) {
        let mut sink = self.sink.lock().await;
        let header = Header {
            system_id,
            component_id,
            sequence: sink.sequence,
        };
        let packet = Packet { header, message };
        sink.sequence = sink.sequence.wrapping_add(1);

        // Silently fail if anything goes wrong.
        //
        // TODO: This should return a result.
        let _ = sink.send(packet).await;
    }

    async fn receiver<U>(mut stream: U, publisher: Publisher)
    where
        U: Stream<Item = DecoderResult> + Unpin,
    {
        loop {
            match stream.next().await {
                Some(result) => match result {
                    Ok(packet) => {
                        // Publish the packet. Stop the task if it fails,
                        // it means that all receivers (including the link)
                        // are dropped.
                        if let Err(_) = publisher.send(packet) {
                            break;
                        }
                    }
                    Err(error) => {
                        eprintln!("[INVALID_PACKET_RECEIVED] {:?}", error);
                    }
                },
                None => {
                    eprintln!("[NONE_RECEIVED] Do something?");
                }
            }
        }
    }
}

type DecoderResult = std::result::Result<Packet, std::io::Error>;

type Publisher = watch::Sender<Packet>;

#[derive(Clone)]
pub struct Subscriber(watch::Receiver<Packet>);

impl Subscriber {
    pub async fn wait_for<F>(&mut self, mut f: F) -> Result<Packet, RecvError>
    where
        F: FnMut(&Packet) -> bool,
    {
        loop {
            self.0.changed().await?;
            let packet = self.0.borrow();
            if f(&packet) {
                break Ok(packet.clone());
            }
        }
    }

    pub async fn wait_for_message(&mut self, id: MessageId) -> Result<Packet, RecvError> {
        self.wait_for(|packet| packet.message.message_id() == id)
            .await
    }

    pub async fn wait_for_ack(&mut self, cmd: MavCmd) -> Result<Packet, RecvError> {
        self.wait_for(|packet| match packet.message {
            Message::COMMAND_ACK(ref ack) => ack.command == cmd,
            _ => false,
        })
        .await
    }
}

impl Deref for Subscriber {
    type Target = watch::Receiver<Packet>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Subscriber {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

struct SequencedSink<T> {
    sink: T,
    sequence: u8,
}

impl<T> Deref for SequencedSink<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.sink
    }
}

impl<T> DerefMut for SequencedSink<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sink
    }
}
