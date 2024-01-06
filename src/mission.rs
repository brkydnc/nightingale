use crate::dialect::{
    MavCmd,
    MavFrame,
    MISSION_ITEM_INT_DATA as RawMissionItem,
};

pub struct MissionPlanner {
    items: Vec<MissionItem>,
}

impl MissionPlanner {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add(mut self, item: MissionItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn get(&self, system: u8, component: u8) -> Vec<RawMissionItem> {
        self.items
            .iter()
            .enumerate()
            .map(|(seq, item)| item.raw(system, component, seq as u16))
            .collect()
    }
}

pub enum MissionItem {
    Waypoint(f32, f32, f32),
    Takeoff(f32, f32, f32),
    ReturnToLaunch,
}

impl MissionItem {
    fn raw(&self, system: u8, component: u8, seq: u16) -> RawMissionItem {
        use MissionItem::*;

        fn scale(f: f32) -> i32 { (f * 1e7) as i32 }

        match self {
            &Waypoint(lat, lon, alt) => RawMissionItem {
                seq,
                command: MavCmd::MAV_CMD_NAV_WAYPOINT,
                param4: f32::NAN,
                x: scale(lat),
                y: scale(lon),
                z: alt,
                autocontinue: true as u8,
                target_system: system,
                target_component: component,
                frame: MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT_INT,
                ..Default::default()
            },
            &Takeoff(lat, lon, alt) => RawMissionItem {
                seq,
                command: MavCmd::MAV_CMD_NAV_TAKEOFF,
                param4: f32::NAN,
                x: scale(lat),
                y: scale(lon),
                z: alt,
                autocontinue: true as u8,
                target_system: system,
                target_component: component,
                frame: MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT_INT,
                ..Default::default()
            },
            &ReturnToLaunch => RawMissionItem {
                seq,
                command: MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
                autocontinue: true as u8,
                target_system: system,
                target_component: component,
                frame: MavFrame::MAV_FRAME_MISSION,
                ..Default::default()
            }
        }
    }
}
