use crate::{ dialect::{Header, Message}, wire::Packet };

use async_broadcast::{self as broadcast };
use flume::{SendError, r#async::SendSink};
use std::{pin::Pin, task::{Context, Poll}, sync::Arc};
use futures::{Future, future::join, Sink, Stream, StreamExt};
use pin_project::pin_project;

#[derive(Clone)]
#[pin_project]
pub struct Link {
    #[pin]
    sender: SendSink<'static, Message>,

    #[pin]
    subscriber: broadcast::Receiver<Arc<Packet>>,
}

impl Link {
    pub fn new<T, U>(
        outgoing: T,
        incoming: U,
        system_id: u8,
        component_id: u8,
    ) -> (Link, impl Future<Output = (Result<(), T::Error>, ())>)
        where T: Sink<Packet>,
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
            async move { let _ = publisher.broadcast(packet).await; }
        });

        // Stamp packets with a sequence number, and forward them to the sink.
        let mut sequence = 0;
        let forward = receiver
            .into_stream()
            .map(move |message| {
                let header = Header {component_id, system_id, sequence };
                sequence += 1;
                Ok(Packet { header, message })
            })
            .forward(outgoing);

        let fut = join(forward, broadcast);
        let link = Link { sender: sender.into_sink(), subscriber };

        (link, fut)
    }
}

impl Sink<Message> for Link {
    type Error = SendError<Message>;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sender.poll_ready(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sender.poll_close(cx)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sender.poll_flush(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        self.project().sender.start_send(item)
    }
}

impl Stream for Link {
    type Item = Arc<Packet>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().subscriber.poll_next(cx)
    }
}
