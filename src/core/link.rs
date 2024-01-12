use crate::{
    dialect::{Header, Message},
    error::{Error, Result},
    wire::Packet,
};
use async_broadcast::{self as broadcast};
use futures::{
    future::{join, Future, FutureExt},
    Sink, Stream, StreamExt,
};
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

#[derive(Clone)]
pub struct Link {
    system_id: u8,
    component_id: u8,
    subscriber: broadcast::Receiver<Arc<Packet>>,
    sender: flume::Sender<Message>,
}

impl Link {
    pub fn new<T, U>(
        outgoing: T,
        incoming: U,
        system_id: u8,
        component_id: u8,
    ) -> (Link, impl Future<Output = ()>)
    where
        T: Sink<Packet>,
        U: Stream<Item = Packet>,
    {
        let (sender, receiver) = flume::bounded(64);
        let (mut publisher, subscriber) = broadcast::broadcast(64);

        // Remove the oldest message if the channel is full.
        publisher.set_overflow(true);

        // Broadcast each incoming message.
        let shared = Arc::new(publisher);
        let broadcast = incoming.for_each(move |packet| {
            let publisher = shared.clone();
            let packet = Arc::new(packet);
            async move {
                let _ = publisher.broadcast(packet).await;
            }
        });

        // Stamp packets with a sequence number, and forward them to the sink.
        let mut sequence = 0;
        let forward = receiver
            .into_stream()
            .map(move |message| {
                let header = Header {
                    component_id,
                    system_id,
                    sequence,
                };
                sequence += 1;
                Ok(Packet { header, message })
            })
            .forward(outgoing);

        let fut = join(forward, broadcast).map(|_| ());
        let link = Link {
            sender,
            subscriber,
            system_id,
            component_id,
        };

        (link, fut)
    }

    pub async fn send_message(&self, message: Message) -> Result<()> {
        self.sender.send_async(message).await.map_err(From::from)
    }

    pub fn system_id(&self) -> u8 {
        self.system_id
    }

    pub fn component_id(&self) -> u8 {
        self.component_id
    }
}

impl Sink<Message> for Link {
    type Error = Error;

    fn start_send(
        self: Pin<&mut Self>,
        item: Message,
    ) -> std::prelude::v1::Result<(), Self::Error> {
        Pin::new(&mut self.get_mut().sender.sink())
            .start_send(item)
            .map_err(From::from)
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::prelude::v1::Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().sender.sink())
            .poll_close(cx)
            .map_err(From::from)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::prelude::v1::Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().sender.sink())
            .poll_flush(cx)
            .map_err(From::from)
    }

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::prelude::v1::Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().sender.sink())
            .poll_ready(cx)
            .map_err(From::from)
    }
}

impl Stream for Link {
    type Item = Arc<Packet>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().subscriber).poll_next(cx)
    }
}
