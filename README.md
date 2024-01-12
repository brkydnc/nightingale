## Arm status
```
const bool armable = sys_status.onboard_control_sensors_health & MAV_SYS_STATUS_PREARM_CHECK;
const bool armed = heartbeat.base_mode & MAV_MODE_FLAG_SAFETY_ARMED
```

## Health status
```
The setters for the falgs below start with `set_health_...(bool ok)`

_health.is_gyrometer_calibration_ok = 
    sys_status.onboard_control_sensors_present & MAV_SYS_STATUS_SENSOR_3D_GYRO

_health.is_accelerometer_calibration_ok =
    sys_status.onboard_control_sensors_health & MAV_SYS_STATUS_SENSOR_3D_ACCEL

_health.is_magnetometer_calibration_ok = 
    sys_status.onboard_control_sensors_present & MAV_SYS_STATUS_SENSOR_3D_MAG

_health.is_global_position_ok
    sys_status_present_enabled_health(sys_status, MAV_SYS_STATUS_SENSOR_GPS);

_health.is_local_position_ok = 
    sys_status_present_enabled_health(sys_status, MAV_SYS_STATUS_SENSOR_OPTICAL_FLOW) ||
    sys_status_present_enabled_health(sys_status, MAV_SYS_STATUS_SENSOR_VISION_POSITION);
    
_health.is_home_position_ok = *Get this from HOME_POSITION message*
```

## Battery status 
```
new_battery.voltage_v = sys_status.voltage_battery * 1e-3f;
new_battery.remaining_percent = sys_status.battery_remaining;
```
