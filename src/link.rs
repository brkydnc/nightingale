use crate::wire::{Packet, Header, Message};
use tokio::sync::{Mutex, watch};
use futures::prelude::*;
use std::ops::{Deref, DerefMut};

type DecoderResult = std::result::Result<Packet, std::io::Error>;

type Publisher = watch::Sender<Packet>;
type Subscriber = watch::Receiver<Packet>;

pub struct Link<T> {
    sink: Mutex<SequencedSink<T>>,
    subscriber: Subscriber,
}

impl<T: Sink<Packet> + Unpin + 'static> Link<T> {
    pub fn new<U>(sink: T, stream: U) -> Self 
        where U: Stream<Item = DecoderResult> + Unpin + Send + 'static,
    {
        let (publisher, subscriber) = watch::channel(Default::default());
        let sink = SequencedSink { sink, sequence: 0 }.into();
        tokio::spawn(Self::receiver(stream, publisher));
        Self { sink, subscriber }
    }

    pub fn subscribe(&self) -> Subscriber {
        self.subscriber.clone()
    }

    pub async fn send(&self, system_id: u8, component_id: u8, message: Message) {
        let mut sink = self.sink.lock().await;
        let header = Header { system_id, component_id, sequence: sink.sequence };
        let packet = Packet { header, message };
        sink.sequence = sink.sequence.wrapping_add(1);

        // Silently fail if anything goes wrong.
        // 
        // TODO: This should return a result.
        let _ = sink.send(packet).await;
    }

    async fn receiver<U>(mut stream: U, publisher: Publisher) 
        where U: Stream<Item = DecoderResult> + Unpin,
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
                    },
                    Err(error) => {
                        eprintln!("[INVALID_PACKET_RECEIVED] {:?}", error);
                    }
                },
                None => {
                    eprintln!("[NONE_RECEIVED] Do something?");
                },
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


