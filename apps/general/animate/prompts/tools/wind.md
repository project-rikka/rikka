+++
name = "Wind"

[params]
frame = "The frame to read; the playhead's when omitted."
+++

Reads the scene's wind as JSON: its value at the frame for each setting, the resting values used where a setting has no keys, each setting's keys with frame, value and curve, and the direction it blows toward as a unit vector.

- Settings: {{wind_settings}}.
- `yaw` 0 blows toward -Z, away from the front of a figure facing +Z; positive yaw turns counter-clockwise seen from above, so 90 blows toward -X. Positive `pitch` blows upward.
- `speed` is in metres per second, `turbulence` 0 to 1 roughens it, `gust` is how often it surges, in hertz.
