use crate::{
    dialect::{Header, Message},
    wire::Packet,
};

use std::{pin::Pin, task::{Context, Poll}, sync::Arc};
use tokio::sync::{ mpsc, broadcast };
use tokio_stream::wrappers::{ReceiverStream, BroadcastStream, errors::BroadcastStreamRecvError};
use tokio_util::sync::{ PollSender, PollSendError };
use futures::{Future, future::join, Sink, Stream, StreamExt};
use pin_project::pin_project;

#[pin_project]
pub struct Link {
    #[pin]
    sender: PollSender<(u8, u8, Message)>,

    #[pin]
    subscriber: BroadcastStream<Arc<Packet>>,
}

impl Link {
    pub fn new<T, U>(
        outgoing: T,
        incoming: U
    ) -> (Link, impl Future<Output = (Result<(), T::Error>, ())>)
        where T: Sink<Packet>,
              U: Stream<Item = Packet>,
    {
        let (sender, receiver) = mpsc::channel(64);
        let (publisher, subscriber) = broadcast::channel(64);

        // Stamp packets with a sequence number, and forward them to the sink.
        let mut sequence = 0;
        let forward = ReceiverStream::new(receiver)
            .map(move |(component_id, system_id, message)| {
                let header = Header {component_id, system_id, sequence };
                sequence += 1;
                Ok(Packet { header, message })
            })
            .forward(outgoing);

        // Broadcast each incoming message.
        let shared = Arc::new(publisher);
        let broadcast = incoming.for_each(move |packet| {
            let publisher = shared.clone();
            let packet = Arc::new(packet);
            async move { let _ = publisher.send(packet); }
        });

        let fut = join(forward, broadcast);
        let link = Link {
            sender: PollSender::new(sender),
            subscriber: BroadcastStream::new(subscriber)
        };

        (link, fut)
    }
}

impl Sink<(u8, u8, Message)> for Link {
    type Error = PollSendError<(u8, u8, Message)>;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sender.poll_ready(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sender.poll_close(cx)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sender.poll_flush(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: (u8, u8, Message)) -> Result<(), Self::Error> {
        self.project().sender.start_send(item)
    }
}

impl Stream for Link {
    type Item = Result<Arc<Packet>, BroadcastStreamRecvError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().subscriber.poll_next(cx)
    }
}
