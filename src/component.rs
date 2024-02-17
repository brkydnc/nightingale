use crate::{
    dialect::{
        COMMAND_INT_DATA as CommandInt,
        COMMAND_LONG_DATA as CommandLong,
        *
    },
    error::{Error, Result},
    link::Link,
    mission::IntoMissionItem,
    wire::Packet,
};

use std::{
    sync::Arc,
    time::Duration,
    result::Result as StdResult,
    pin::Pin,
    task::{Poll, Context}
};

use futures_util::{pin_mut, future::{Future, Ready, ready}, Stream, StreamExt};
use futures_time::{
    stream::StreamExt as FuturesTimeStreamExt,
    time::Duration as FuturesTimeDuration,
};

pub use async_broadcast::TryRecvError;

const MAX_RETRY: usize = 5;
const MAX_CONFIRMATION: u8 = 5;
const ACK_TIMEOUT: Duration = Duration::from_millis(1500);
const LONG_TIMEOUT: Duration = Duration::from_millis(6000);

#[derive(Clone)]
pub struct Component {
    id: u8,
    system: u8,
    link: Link,
}

impl Component {
    pub fn new(id: u8, system: u8, link: Link) -> Self {
        Self { id, system, link }
    }

    pub fn try_recv(&mut self) -> StdResult<Arc<Packet>, TryRecvError> {
        loop {
            let packet = self.link.subscriber.try_recv()?;
            let Header { system_id, component_id, .. } = packet.header;

            if system_id == self.system && component_id == self.id {
                break Ok(packet)
            }
        }
    }

    async fn _timeout<T, F, Fut>(&mut self, f: F, dur: Duration) -> Result<T>
    where F: FnMut(Arc<Packet>)-> Fut,
          Fut: Future<Output = Option<T>>
    {
        let fut = self
            .filter_map(f)
            .timeout(FuturesTimeDuration::from(dur));

        pin_mut!(fut);

        fut
            .next()
            .await
            .ok_or(Error::Closed)?
            .or(Err(Error::Timeout))
    }

    async fn probe<T, F, Fut>(&mut self, mut f: F, dur: Duration, mut retry: usize) -> Result<T>
    where F: FnMut(Arc<Packet>)-> Fut,
          Fut: Future<Output = Option<T>>
    {
        while retry > 0 {
            match self._timeout(&mut f, dur).await {
                Err(Error::Timeout) => { retry -= 1 },
                other => return other,
            }
        }

        Err(Error::Timeout)
    }

    // TODO: We need packet routing (target system id matches, blah blah).
    pub async fn command_int(&mut self, mut command: CommandInt) -> Result<MavResult> {
        command.target_system = self.system;
        command.target_component = self.id;

        let ref filter = ack_filter(command.command);

        self.link.send_message(Message::COMMAND_INT(command)).await?;
        self.probe(filter, ACK_TIMEOUT, MAX_RETRY).await
    }

    // TODO: We need packet routing (target system id matches, blah blah).
    pub async fn command_long(&mut self, mut command: CommandLong) -> Result<MavResult> {
        // Slap the target address.
        command.target_system = self.system;
        command.target_component = self.id;

        // Create a filter for ack commands that will catch the current command.
        let ref filter = ack_filter(command.command);
        let message = Message::COMMAND_LONG(command);
        let mut confirmation = 0;

        // Send command with increasing confirmation until we receive an ACK.
        while confirmation < MAX_CONFIRMATION {
            // Send the command.
            self.link.send_message(message.clone()).await?;

            // Wait for an ack, timeout after a certain time.
            match self._timeout(filter, ACK_TIMEOUT).await {
                // When we receive a progress, we will wait for an ending ack.
                Ok(MavResult::MAV_RESULT_IN_PROGRESS) => loop {
                    match self._timeout(filter, LONG_TIMEOUT).await {
                        Ok(MavResult::MAV_RESULT_IN_PROGRESS) => { }
                        other => return other,
                    }
                }

                // If timed out, increased the confirmation field and retry.
                Err(Error::Timeout) => { confirmation += 1 }

                // Return what we received, this can be an error.
                other => return other,
            }
        }

        // Fallback for maximum number of confirmations.
        Err(Error::Timeout)
    }

    pub async fn start_mission(&mut self) -> Result<MavResult> {
        self.command_long(CommandLong {
            command: MavCmd::MAV_CMD_MISSION_START,
            ..Default::default()
        }).await
    }

    pub async fn upload_mission<M, I>(&mut self, mission: M) -> Result<MavMissionResult>
    where
        M: AsRef<[I]>,
        I: IntoMissionItem,
    {
        let items = mission.as_ref();

        let mission_count = Message::MISSION_COUNT(MISSION_COUNT_DATA {
            count: items.len() as u16,
            target_system: self.system,
            target_component: self.id,
            mission_type: MavMissionType::MAV_MISSION_TYPE_MISSION,
        });

        self.link.send_message(mission_count).await?;

        let mission_result = loop {
            let packet = self.probe(mission_filter, ACK_TIMEOUT, MAX_RETRY).await?;

            match &packet.message {
                Message::MISSION_REQUEST(req) => {
                    let item = items[req.seq as usize].with(self.system, self.id, req.seq);

                    let mission_item = Message::MISSION_ITEM(item);
                    self.link.send_message(mission_item).await?;
                }
                Message::MISSION_REQUEST_INT(req) => {
                    let item = items[req.seq as usize].with_int(self.system, self.id, req.seq);

                    let mission_item = Message::MISSION_ITEM_INT(item);
                    self.link.send_message(mission_item).await?;
                }
                Message::MISSION_ACK(ack) => {
                    break ack.mavtype;
                }
                _ => unreachable!(),
            }
        };

        Ok(mission_result)
    }

    pub async fn set_mode(&mut self, mode: CopterMode) -> Result<MavResult> {
        self.command_long(CommandLong {
            command: MavCmd::MAV_CMD_DO_SET_MODE,
            param1: MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED.bits() as f32,
            param2: mode as u32 as f32,
            ..Default::default()
        }).await
    }

    pub async fn set_message_interval(&mut self, id: MessageId, interval: Duration) -> Result<MavResult> {
        self.command_long(CommandLong {
            command: MavCmd::MAV_CMD_SET_MESSAGE_INTERVAL,
            param1: id as f32,
            param2: interval.as_micros() as f32,
            ..Default::default()
        }).await
    }

    pub async fn arm(&mut self, armed: bool) -> Result<MavResult> {
        self.command_long(CommandLong {
            param1: armed as u8 as f32,
            command: MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
            ..Default::default()
        }).await
    }

    pub async fn wait_armable(&mut self) -> bool {
        self.any(|packet| async move {
            match &packet.message {
                Message::SYS_STATUS(status) => {
                    status
                        .onboard_control_sensors_health
                        .contains(MavSysStatusSensor::MAV_SYS_STATUS_PREARM_CHECK)
                },
                _ => false
            }
        }).await
    }

    pub async fn wait_armed(&mut self) -> bool {
        self.any(|packet| async move {
            match &packet.message {
                Message::HEARTBEAT(heartbeat) => {
                    heartbeat
                        .base_mode
                        .contains(MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED)
                },
                _ => false
            }
        }) .await
    }

    pub async fn manual_control(&mut self, mut data: MANUAL_CONTROL_DATA) -> Result<()> {
        data.target = self.system;
        self.link.send_message(Message::MANUAL_CONTROL(data)).await
    }
}

impl Stream for Component {
    type Item = Arc<Packet>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            match Pin::new(&mut this.link).poll_next(cx) {
                Poll::Ready(Some(packet)) => {
                    let Header { system_id, component_id, .. } = packet.header;

                    if system_id == this.system && component_id == this.id {
                        break Poll::Ready(Some(packet))
                    }
                },
                other => break other,
            }
        }
    }
}

fn mission_filter(packet: Arc<Packet>) -> Ready<Option<Arc<Packet>>> {
    use Message::{
        MISSION_ACK as Ack,
        MISSION_REQUEST as Request,
        MISSION_REQUEST_INT as RequestInt,
    };

    match &packet.message {
        Request(_) | RequestInt(_) | Ack(_) => ready(Some(packet)),
        _ => ready(None)
    }
}

fn ack_filter(command: MavCmd) -> impl Fn(Arc<Packet>) -> Ready<Option<MavResult>> {
    move |packet| {
        if let Message::COMMAND_ACK(ack) = &packet.message {
            ready(command.eq(&ack.command).then_some(ack.result))
        } else {
            ready(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test whether the component streams *only* the packets that carry its 
    // system and component ids.
    #[tokio::test]
    async fn component_streams_packets() {
        let header = Header { component_id: 1, system_id: 1, sequence: 0, };
        let unrecognized = Header { component_id: 2, system_id: 2, sequence: 0 };

        let messages = [
            Message::HEARTBEAT(Default::default()),
            Message::GLOBAL_POSITION_INT(Default::default()),
            Message::SYS_STATUS(Default::default()),
            Message::TIMESYNC(Default::default()),
            Message::HEARTBEAT(Default::default()),
        ];

        let number_of_messages_to_be_received = messages.len() * 2;

        let component_packets = messages
            .clone()
            .into_iter()
            .map(|message| Packet { header, message } );

        let unrecognized_component_packets = messages
            .into_iter()
            .map(|message| Packet { header: unrecognized, message } );

        let component_packets_continued = component_packets.clone();

        let packets = component_packets
            .chain(unrecognized_component_packets)
            .chain(component_packets_continued);

        let stream = futures::stream::iter(packets);
        let sink = futures::sink::drain();

        let (link, connection) = Link::new(sink, stream, 0, 0);
        let component = Component::new(1, 1, link);

        let receive = component.count();

        let (_, count) = futures::future::join(connection, receive).await;

        assert_eq!(number_of_messages_to_be_received, count);
    }
}
