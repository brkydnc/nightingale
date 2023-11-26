use crate::error::Result;
use std::collections::HashMap;
use mavlink::Message;
use tokio::{select, net::ToSocketAddrs, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use crate::connection::{ Connection, Receiver };

type MessageID = u32;
type MessageHandler<M> = Box<dyn Fn(&M) + Send>;
type MessageHandlerMap<M> = HashMap<MessageID, Vec<MessageHandler<M>>>;

pub struct IdleLink<M, C: Connection> {
    sender: C::Sender, 
    receiver: C::Receiver,
    handlers: MessageHandlerMap<M>,
}

impl<M: Message + 'static, C: Connection + 'static> IdleLink<M, C> {
    pub fn start(self) -> Link<M, C> {
        let receiver = self.receiver;
        let task = ReceiverTask::spawn::<M>(receiver);

        Link {
            sender: self.sender,
            handlers: self.handlers,
            task,
        }
    }
}

pub struct Link<M, C: Connection> {
    sender: C::Sender, 
    handlers: MessageHandlerMap<M>,
    task: ReceiverTask<C::Receiver>,
}

impl<M: Message, C: Connection> Link<M, C> {
    pub async fn connect<A>(address: A) -> Result<IdleLink<M, C>>
        where A: ToSocketAddrs + Send
    {
        let (sender, receiver) = C::connect(address).await?;
        Ok(IdleLink { sender, receiver, handlers: Default::default() })
    }

    // fn subscribe(&mut self, id: MessageID, handler: MessageHandler<T>) {
    //     self.handlers.entry(id).or_default().push(handler);
    // }
}

impl<M, C: Connection> Drop for Link<M, C> {
    fn drop(&mut self) {
        self.task.token.cancel();
    }
}

struct ReceiverTask<R> {
    handle: JoinHandle<R>,
    token: CancellationToken,
}

impl<R: Receiver + 'static> ReceiverTask<R> {
    fn spawn<M>(receiver: R) -> Self
        where M: Message + 'static
    {
        let token = CancellationToken::new();
        let fut = Self::receive::<M>(receiver, token.clone());
        let handle = tokio::spawn(fut);
        Self { handle, token }
    }

    async fn receive<M: Message>(mut receiver: R, token: CancellationToken) -> R {
        loop {
            select! {
                _ = token.cancelled() => { break }
                result = receiver.receive::<M>() => {
                    match result {
                        Ok(message) => { dbg!(message.message_id()); }
                        Err(err) => { dbg!(err); }
                    }
                }
            }
        }

        receiver
    }
}
