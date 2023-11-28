use crate::connection::{Connection, Receiver};
use crate::error::Result;
use mavlink::Message;
use std::ops::Deref;
use tokio::{net::ToSocketAddrs, select};
use tokio_util::sync::CancellationToken;

pub struct Link<C: Connection> {
    sender: C::Sender,
    token: CancellationToken,
}

impl<C: Connection + 'static> Link<C> {
    pub async fn connect<M, D, A>(address: A, dispatcher: D) -> Result<Self>
    where
        M: Message + 'static,
        D: Fn(M) + Send + 'static,
        A: ToSocketAddrs + Send,
    {
        let (sender, receiver) = C::connect(address).await?;
        let token = CancellationToken::new();
        let fut = Self::receive::<M, D>(receiver, dispatcher, token.clone());

        tokio::spawn(fut);

        Ok(Self { sender, token })
    }

    async fn receive<M, D>(mut receiver: C::Receiver, dispatcher: D, token: CancellationToken)
    where
        M: Message,
        D: Fn(M),
    {
        loop {
            select! {
                _ = token.cancelled() => { break }
                result = receiver.receive::<M>() => {
                    match result {
                        Ok(message) => { dispatcher(message) }
                        Err(err) => { dbg!(err); }
                    }
                }
            }
        }
    }
}

impl<C: Connection> Deref for Link<C> {
    type Target = C::Sender;

    fn deref(&self) -> &Self::Target {
        &self.sender
    }
}

impl<C: Connection> Drop for Link<C> {
    fn drop(&mut self) {
        // TODO: We cancel, but not join the receiver task. Is this problematic?
        self.token.cancel();
    }
}
