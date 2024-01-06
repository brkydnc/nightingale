enum MissionItem {
    Waypoint(f32, f32, f32),
    Takeoff(f32, f32, f32),
    ReturnToLaunch,
}

impl MissionItem {
    fn raw(self) -> RawMissionItem {
        use MissionItem::*;

        fn scale(f: f32) -> i32 { (f * 1e7) as i32 }

        match self {
            Waypoint(lat, lon, alt) => RawMissionItem {
                command: MavCmd::MAV_CMD_NAV_WAYPOINT,
                param4: f32::NAN,
                x: scale(lat),
                y: scale(lon),
                z: alt,
                autocontinue: true as u8,
                target_system: TARGET_SYSTEM_ID,
                target_component: TARGET_COMPONENT_ID,
                frame: MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT_INT,
                ..Default::default()
            },
            Takeoff(lat, lon, alt) => RawMissionItem {
                command: MavCmd::MAV_CMD_NAV_TAKEOFF,
                param4: f32::NAN,
                x: scale(lat),
                y: scale(lon),
                z: alt,
                autocontinue: true as u8,
                target_system: TARGET_SYSTEM_ID,
                target_component: TARGET_COMPONENT_ID,
                frame: MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT_INT,
                ..Default::default()
            },
            ReturnToLaunch => RawMissionItem {
                command: MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
                autocontinue: true as u8,
                target_system: TARGET_SYSTEM_ID,
                target_component: TARGET_COMPONENT_ID,
                frame: MavFrame::MAV_FRAME_MISSION,
                ..Default::default()
            }
        }
    }
}

struct MissionPlanner {
    items: Vec<RawMissionItem>,
}

impl MissionPlanner {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    fn add(mut self, item: MissionItem) -> Self {
        let mut raw = item.raw();
        raw.seq = self.items.len() as u16;
        self.items.push(raw);
        self
    }
