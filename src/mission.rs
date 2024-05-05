use crate::dialect::{
    MavCmd, MavFrame, MISSION_ITEM_DATA as RawMissionItem,
    MISSION_ITEM_INT_DATA as RawMissionItemInt,
};

pub trait IntoMissionItem {
    fn raw(&self) -> RawMissionItem;

    fn int(&self) -> RawMissionItemInt {
        fn scale(f: f32) -> i32 {
            (f * 1e7) as i32
        }

        let mut int: RawMissionItemInt = Default::default();
        let raw = self.raw();

        int.param1 = raw.param1;
        int.param2 = raw.param2;
        int.param3 = raw.param3;
        int.param4 = raw.param4;
        int.x = scale(raw.x);
        int.y = scale(raw.y);
        int.z = raw.z;
        int.seq = raw.seq;
        int.command = raw.command;
        int.target_system = raw.target_system;
        int.target_component = raw.target_component;
        int.frame = raw.frame;
        int.current = raw.current;
        int.autocontinue = raw.autocontinue;

        int
    }

    fn with(&self, system: u8, component: u8, seq: u16) -> RawMissionItem {
        let mut item = self.raw();
        item.seq = seq;
        item.target_system = system;
        item.target_component = component;

        item
    }

    fn with_int(&self, system: u8, component: u8, seq: u16) -> RawMissionItemInt {
        let mut item = self.int();
        item.seq = seq;
        item.target_system = system;
        item.target_component = component;

        item
    }
}

pub enum MissionItem {
    Waypoint(f32, f32, f32),
    Takeoff(f32, f32, f32),
    ReturnToLaunch,
    DoChangeSpeed,
}

impl IntoMissionItem for MissionItem {
    fn raw(&self) -> RawMissionItem {
        use MissionItem::*;

        match *self {
            Waypoint(lat, lon, alt) => RawMissionItem {
                command: MavCmd::MAV_CMD_NAV_WAYPOINT,
                param4: f32::NAN,
                x: lat,
                y: lon,
                z: alt,
                autocontinue: true as u8,
                frame: MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT,
                ..Default::default()
            },
            Takeoff(lat, lon, alt) => RawMissionItem {
                command: MavCmd::MAV_CMD_NAV_TAKEOFF,
                param4: f32::NAN,
                x: lat,
                y: lon,
                z: alt,
                autocontinue: true as u8,
                frame: MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT,
                ..Default::default()
            },
            ReturnToLaunch => RawMissionItem {
                command: MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
                autocontinue: true as u8,
                frame: MavFrame::MAV_FRAME_MISSION,
                ..Default::default()
            },
            DoChangeSpeed => RawMissionItem {
                command: MavCmd::MAV_CMD_DO_CHANGE_SPEED,
                param1: 0.0,
                param2: 0.8,
                param3: -2.0,
                ..Default::default()
            },
        }
    }
}
