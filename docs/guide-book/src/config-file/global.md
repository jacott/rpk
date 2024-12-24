# Global Section

Keyboard wide values are defined in this section and can contain any of the following options:

#### `dual_action_timeout = <milliseconds>`
How long to wait for a tap to occur on an overloaded key. `overload_tap_timeout` is an aliases for
this option. The default is 180ms.

#### `dual_action_timeout2 = <milliseconds>`
How long to wait after another two keys are pressed (or released) before giving up on waiting for a
tap. The default is 20ms.

#### `tapdance_tap_timeout = <milliseconds>>` {#tapdance_tap_timeout}
How long to wait between taps before executing the counted [`tapdance`][tapdance] action. The timer
restarts after every key press.

#### `debounce_settle_time = <milliseconds>`
How long to wait for a key press or release to settle before reporting the next change in state. The
timer starts from the last bounce detected; so a noisy key will take longer to settle than a stable
key. The default is 20ms. In essence this dictates the minimum report time between a key press and
release or release and press. Allowed values range from 0.1ms to 25.0ms.

<div class="warning">
All keys on the keyboard must be released for at least 2 seconds for a change in value of this
field to take effect.
</div>

#### `unicode_prefix = <action>`
The action to run before sending a unicode sequence.

#### `unicode_suffix = <action>`
The action to run after sending a unicode sequence.

#### `[global.mouse_profile<n>.movement] (or .scroll)`
Where `<n>` may be 1, 2, or 3. Is a subsection detailing the acceleration profile of the mouse
movement (or mouse scroll). The following subfields are allowed:

- **`curve = [<s>, <e>]`**: where `<s>` and `<e>` are floating point numbers specifying the "x"
      part of the control points of a bezier curve (0 to 1). If `<s>` is 0 then the accerlation is
      slow to change in the begining; if it is 1 then it is fast to change at the start. Conversely
      if `<e>` is 0 then the accerlation is fast to change at the end; if it is 1 then slow to
      change at the end.
- **`max_time = <milliseconds>`**: How long it takes to get to the end of the bezier curve.
- **`min_ticks_per_ms = <milliseconds>`**: The absolute minimum speed of the mouse.
- **`max_ticks_per_ms = <milliseconds>`**: The absolute maximum speed of the mouse.

## Example

```ini
[global]

unicode_prefix          = C-S-u
unicode_suffix          = macro(return delay(20))
overload_tap_timeout    = 180

[global.mouse_profile1.movement]

curve                   = [.2, 1]
max_time                = 1000
min_ticks_per_ms        = .1
max_ticks_per_ms        = 5

[global.mouse_profile1.scroll]

curve                   = [1, 0]
max_time                = 5000
min_ticks_per_ms        = .01
max_ticks_per_ms        = .1
```

[tapdance]: actions.md#tapdance
