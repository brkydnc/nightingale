use crate::{
    dialect::{Header, Message},
    error::{Error, Result},
    wire::Packet,
};
use async_broadcast::{self as broadcast};
use futures::{
    pin_mut,
    future::{join, Future, FutureExt},
    Sink, SinkExt, Stream, StreamExt,
};
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

#[derive(Clone)]
pub struct Link {
    subscriber: broadcast::Receiver<Arc<Packet>>,
    sender: flume::Sender<Message>,
}

impl Link {
    /// Constructs a new `Link` for the given `Sink` and `Stream` interfaces.
    ///
    /// This function returns a `Link` and a future. The future must be spawned
    /// in order to forward outgoing messages, and broadcast incoming messages
    /// through the `Link`.
    ///
    /// The returned future will not resolve until one of the following is true:
    ///
    /// * A broadcast is attempted when all `Link` instances are dropped.
    /// * The given `Sink` encounters an error *and* the `Stream` is exhausted.
    ///
    /// Notice that if the Sink encounters an error, but the `Stream` is not
    /// exhausted, the future will run until the `Stream` is exhausted.
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
        let broadcast = async move {
            // Pin the stream so that we can call `.next()` on it.
            pin_mut!(incoming);

            // Broadcast channel does not implement `Sink`, so instead of forwarding,
            // we somehow need to publish incoming packets, This is why we loop.
            while let Some(packet) = incoming.as_mut().map(Arc::new).next().await {
                if let Err(_) = publisher.broadcast_direct(packet).await {
                    // Publish packets, stop if all receivers are dropped.
                    break;
                }
            }
        };

        let forward = async move {
            let mut header = Header { component_id, system_id, sequence: 255 };
            let mut stream = receiver.into_stream().map(move |message| {
                header.sequence = header.sequence.wrapping_add(1);
                Ok(Packet { header, message })
            });

            // Forward all packets in stream. Do not care how the "forwarding"
            // process ended, whether it is a `Sink::Error` or a `Stream` exhaust.
            pin_mut!(outgoing);
            let _ = outgoing.send_all(&mut stream).await;
        };

        let link = Link { sender, subscriber };
        let fut = join(forward, broadcast).map(|_| ());

        (link, fut)
    }

    pub fn split(self) -> (flume::Sender<Message>, broadcast::Receiver<Arc<Packet>>) {
        (self.sender, self.subscriber)
    }

    pub async fn send_message(&self, message: Message) -> Result<()> {
        self.sender.send_async(message).await.map_err(From::from)
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::task::Context;

    #[test]
    fn link_future_resolves_when_all_links_are_dropped() {
        let sink = futures::sink::drain();
        let stream = futures::stream::repeat(Default::default());
        let (link, fut) = Link::new(sink, stream, 0, 0);

        let waker = futures::task::noop_waker();
        let mut context = Context::from_waker(&waker);

        drop(link);

        pin_mut!(fut);
        match fut.poll(&mut context) {
            Poll::Ready(_) => { },
            Poll::Pending => unreachable!(),
        }
    }

    // TODO: Test the case where sink and stream end, and the future is resolved.
}
