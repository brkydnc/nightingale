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
use futures::{future::ready, StreamExt};
use futures_time::{
    stream::StreamExt as FuturesTimeStreamExt, time::Duration as FuturesTimeDuration,
};
use std::{sync::Arc, time::Duration};

pub struct Component {
    id: u8,
    system: u8,
    link: Link,
}

impl Component {
    pub fn new(id: u8, system: u8, link: Link) -> Self {
        Self { id, system, link }
    }

    async fn timeout<F>(
        &mut self,
        mut filter: F,
        duration: Duration,
        mut retries: usize,
    ) -> Result<Arc<Packet>>
    where
        F: FnMut(&Packet) -> bool,
    {
        while retries > 0 {
            let incoming = (&mut self.link)
                .filter(|p| {
                    ready(p.header.system_id == self.system && p.header.component_id == self.id)
                })
                .filter(|p| ready(filter(p)))
                .timeout(FuturesTimeDuration::from(duration))
                .next()
                .await;

            if let Some(Ok(packet)) = incoming {
                return Ok(packet);
            } else {
                retries -= 1;
            }
        }

        Err(Error::Timeout)
    }

    // TODO: Check command.ack.command == command.
    // TODO: We need packet routing (target system id matches, blah blah).
    // XXX: COMMAND_ACK_DATA Does not include target_system unless it has
    //      serde feature flag.
    pub async fn command_int(&mut self, mut command: CommandInt) -> Result<MavResult> {
        command.target_system = self.system;
        command.target_component = self.id;

        let filter = |packet: &Packet| matches!(packet.message, Message::COMMAND_ACK(_));

        self.link
            .send_message(Message::COMMAND_INT(command))
            .await?;
        let packet = self.timeout(filter, Duration::from_millis(1500), 5).await?;

        match &packet.message {
            Message::COMMAND_ACK(ack) => Ok(ack.result),
            _ => unreachable!(),
        }
    }

    // TODO: Currently, this is how command_int works, implement long command protocol here.
    pub async fn command_long(&mut self, mut command: CommandLong) -> Result<MavResult> {
        command.target_system = self.system;
        command.target_component = self.id;

        let filter = |packet: &Packet| matches!(packet.message, Message::COMMAND_ACK(_));

        self.link
            .send_message(Message::COMMAND_LONG(command))
            .await?;
        let packet = self.timeout(filter, Duration::from_millis(1500), 5).await?;

        match &packet.message {
            Message::COMMAND_ACK(ack) => Ok(ack.result),
            _ => unreachable!(),
        }
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
        use Message::{
            MISSION_ACK as Ack, MISSION_ITEM as Item, MISSION_ITEM_INT as ItemInt,
            MISSION_REQUEST as Request, MISSION_REQUEST_INT as RequestInt,
        };

        let filter = |p: &Packet| matches!(p.message, Request(_) | RequestInt(_) | Ack(_));
        let items = mission.as_ref();

        let mission_count = Message::MISSION_COUNT(MISSION_COUNT_DATA {
            count: items.len() as u16,
            target_system: self.system,
            target_component: self.id,
        });

        self.link.send_message(mission_count).await?;

        let mission_result = loop {
            let packet = self.timeout(filter, Duration::from_millis(1500), 5).await?;

            match &packet.message {
                Request(req) => {
                    let item = items[req.seq as usize].with(self.system, self.id, req.seq);

                    let mission_item = Item(item);
                    self.link.send_message(mission_item).await?;
                }
                RequestInt(req) => {
                    let item = items[req.seq as usize].with_int(self.system, self.id, req.seq);

                    let mission_item = ItemInt(item);
                    self.link.send_message(mission_item).await?;
                }
                Ack(ack) => {
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

    pub async fn arm(&mut self) -> Result<MavResult> {
        self.command_long(CommandLong {
            param1: 1.0,
            command: MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
            ..Default::default()
        }).await
    }

    pub async fn disarm(&mut self) -> Result<MavResult> {
        self.command_long(CommandLong {
            param1: 0.0,
            command: MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
            ..Default::default()
        }).await
    }

    pub async fn wait_armable(&mut self) -> bool {
        (&mut self.link)
            .any(|packet| async move {
                match &packet.message {
                    Message::SYS_STATUS(status) => {
                        status
                            .onboard_control_sensors_health
                            .contains(MavSysStatusSensor::MAV_SYS_STATUS_PREARM_CHECK)
                    },
                    _ => false
                }
            })
            .await
    }

    pub async fn wait_armed(&mut self) -> bool {
        (&mut self.link)
            .any(|packet| async move {
                match &packet.message {
                    Message::HEARTBEAT(heartbeat) => {
                        heartbeat
                            .base_mode
                            .contains(MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED)
                    },
                    _ => false
                }
            })
            .await
    }
}
