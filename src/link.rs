use crate::connection::{Connection, Receiver, Sender};
use crate::error::Result;

use mavlink::Message;
use tokio_util::sync::CancellationToken;

use tokio::{net::ToSocketAddrs, select, sync::Mutex as AsyncMutex};

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub type MessageID = u32;

// TODO: Find a way to unregister handlers using a unique identifier.
// static ID_GENERATOR: AtomicUsize: AtomicUsize::new(0);
// struct MessageHandler { id: usize, f: Box<dyn Fn(&M)> }
pub type MessageHandler<M> = Arc<dyn Fn(&M) + Send + Sync>;

type MessageHandlerMap<M> = HashMap<MessageID, Vec<MessageHandler<M>>>;

pub struct Link<M, C: Connection> {
    sender: AsyncMutex<C::Sender>,
    token: CancellationToken,
    handlers: Arc<Mutex<MessageHandlerMap<M>>>,
}

// TODO: The generics shouldn't be too strict.
impl<M: Message + Sync + 'static, C: Connection + 'static> Link<M, C> {
    pub async fn connect<A>(address: A) -> Result<Self>
    where
        A: ToSocketAddrs + Send,
    {
        let (sender, receiver) = C::connect(address).await?;
        let handlers = Default::default();
        let token = CancellationToken::new();
        let fut = Self::receive(receiver, token.clone(), Arc::clone(&handlers));

        tokio::spawn(fut);

        Ok(Self {
            sender: sender.into(),
            token,
            handlers,
        })
    }

    pub fn register(&self, id: MessageID, handler: MessageHandler<M>) {
        // TODO: Panicking here is probably a bad idea.
        let mut map = self.handlers.lock().expect("Handler map is poisoned.");

        map.entry(id).or_default().push(handler);
    }

    pub fn unregister(&self, id: MessageID, handler: MessageHandler<M>) {
        // TODO: Panicking here is probably a bad idea.
        let mut map = self.handlers.lock().expect("Handler map is poisoned.");

        map.entry(id).and_modify(|vec| {
            let query = vec.iter().position(|h| Arc::ptr_eq(h, &handler));

            if let Some(index) = query {
                vec.swap_remove(index);
            }
        });
    }

    pub async fn send(&self, system: u8, component: u8, message: &M) -> Result<usize> {
        let mut sender = self.sender.lock().await;
        sender.send(system, component, message).await
    }

    async fn receive(
        mut receiver: C::Receiver,
        token: CancellationToken,
        handlers: Arc<Mutex<MessageHandlerMap<M>>>,
    ) {
        loop {
            select! {
                _ = token.cancelled() => { break }
                result = receiver.receive::<M>() => {
                    match result {
                        Ok(message) => {
                            // XXX: Panicking here is very highly likely a bad idea.
                            let mut map = handlers.lock()
                                .expect("Handler map is poisoned.");

                            map
                                .entry(message.message_id())
                                .and_modify(|vec| {
                                    for f in vec {
                                        f(&message);
                                    }
                                });
                        }
                        Err(err) => { dbg!(err); }
                    }
                }
            }
        }
    }
}

impl<M, C: Connection> Drop for Link<M, C> {
    fn drop(&mut self) {
        // TODO: We cancel, but not join the receiver task. Is this problematic?
        self.token.cancel();
    }
}
