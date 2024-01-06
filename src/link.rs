use crate::{
    dialect::{Header, Message},
    error::{ Error, Result },
    wire::{Packet, DecoderResult},
};

use std::{
    sync::Arc,
    time::Duration,
    ops::{Deref, DerefMut}
};

use tokio::sync::{ broadcast, Mutex };
use futures::prelude::*;

// Packets are large. We send and receive an Arc clone instead of the packets.
type Publisher = broadcast::Sender<Arc<Packet>>;

// TODO: Maybe interop with BroadcastStream?
pub struct Subscriber(broadcast::Receiver<Arc<Packet>>);

impl Subscriber {
    pub async fn receive(&mut self) -> Result<Arc<Packet>> {
        self.0.recv().await.map_err(Error::from)
    }

    pub async fn wait<F>(&mut self, mut f: F) -> Result<Arc<Packet>>
    where
        F: FnMut(&Packet) -> bool,
    {
        loop {
            let packet = self.0.recv().await?;
            if f(&packet) {
                break Ok(packet);
            }
        }
    }

    pub async fn timeout<F>(
        &mut self,
        mut f: F,
        duration: Duration,
        retries: usize
    ) -> Result<Arc<Packet>>
    where
        F: FnMut(&Packet) -> bool,
    {
        let mut attempts = 0;

        while attempts < retries {
            match tokio::time::timeout(duration, self.wait(&mut f)).await {
                Ok(result) => { return result; }
                Err(_elapsed) => { attempts += 1; }
            }
        }

        Err(Error::Timeout)
    }
}

pub struct Link<T> {
    // A generic consumer interface for outgoing MAVLink messages.
    sink: Mutex<SequencedSink<T>>,

    // Keep this here, to keep the listen task alive. Even there are no
    // subscribers, the listener task should live until the link is dropped.
    receiver: broadcast::Receiver<Arc<Packet>>,
}

impl<T: Sink<Packet> + Unpin + 'static> Link<T> {
    const CAPACITY: usize = 32;

    pub fn new<U>(sink: T, stream: U) -> Self
    where
        U: Stream<Item = DecoderResult> + Unpin + Send + 'static,
    {
        let (publisher, receiver) = broadcast::channel(Self::CAPACITY);
        let sink = SequencedSink { sink, sequence: 0 }.into();
        tokio::spawn(Self::listen(stream, publisher.clone()));
        Self { sink, receiver }
    }

    pub fn subscribe(&self) -> Subscriber {
        // For the sake of simplicity, we don't have "topics" here yet. All 
        // subscribers receive all type of messages.
        Subscriber(self.receiver.resubscribe())
    }

    pub async fn send(&self, system_id: u8, component_id: u8, message: Message) -> Result<()> {
        let mut sink = self.sink.lock().await;
        let header = Header {
            system_id,
            component_id,
            sequence: sink.sequence,
        };
        let packet = Packet { header, message };
        sink.sequence = sink.sequence.wrapping_add(1);

        // TODO: is impl From<...> for Error possible here?
        sink.send(packet).await.map_err(|_| Error::Send)
    }

    async fn listen<U>(mut stream: U, publisher: Publisher)
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
                        if let Err(_) = publisher.send(Arc::new(packet)) {
                            break;
                        }
                    }
                    Err(error) => {
                        eprintln!("[INVALID_PACKET_RECEIVED] {:?}", error);
                    }
                },
                None => { break }
            }
        }
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
