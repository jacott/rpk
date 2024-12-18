# Actions and Keycodes

## Keycodes

RPK supports most of the keycodes for the [USB HID][1] classes of keyboard, consumer control, system
control, and mouse. The symbolic names of these keycodes vary and RPK supports some common
aliases. To get a list of all the keycode names run the following command:

```sh
rpk-config list-keycodes
```

Try adding `--help` for extra features available for listing the codes.

Other pseduo keycodes are used to change the state of the keyboard.


The special keycodes are:

1. `mouseaccel<n>` where `<n>` is 1, 2, or 3. `mouseaccel1` will change the mouse movement and
   scroll accelerations characteristics from the default `mouseaccel2` as will `mouseaccel3` usually
   1 will be slower and 3 will be faster than the default.
1. `stop_active` will release any held keys and clear the modifier layers.
1. `clear_layers` will deactivate all layers expect the base layout.
1. `clear_all` will stop all actions/macros, clear all layers, restore the base layout to `main`,
   and release all keys.
1. `reset_keyboard` will restart the keyboard firmware as i f it had just been powered on.
1. `reset_to_usb_boot` will restart the keyboard in mass storage mode, if supported, which will
   allow a new firmware binary to be installed.


## Actions

Actions allow for keyboard specific functions to be invoked that take one or more arguments.

#### `layer(<layer>)`

Activate the given layer for the duration of the key press.

#### `oneshot(<layer>)`

When tapped activate the layer for the next key press only.

#### `setlayout(<layout>)`

Replace the base layout.

#### `toggle(<layer>)`

Turn on a layer if inactive; otherwise turn off the layer.

#### `delay(<milliseconds>)`

Wait the given milliseconds before reporting the next keycode to the host computer.

#### `dualaction(<hold-action>, <tap-action>[, <timeout1>[, <timeout2>]])`

Run the `<hold-acton>` when held, execute the `<tap-action>` on tap. `<timeout1>` and `<timeout2>`
override `global.dual_action_timeout` and `global.dual_action_timeout2` respectively. A key is
considered held if `<timeout1>` expired before no more than two other key events happen; or
`<timeout2>` expires before more than two key events happen. `<timeout2>` starts running after two
other key events are detected.

#### `overload(<layer>, <action>[, <timeout1>[, <timeout2>]])`

Overload is an alias for `dualaction(layer(<layer>), <action>[, <timeout1>[, <timeout2>]])`.

## Macros

Macro expressions are user defined actions that run a sequence of other actions/keycodes. The
following forms are all valid macro expressions:

1. `macro(<expr>)`
1. `hold(<expr>)`
1. `release(<expr>)`
1. `<modifier-list>-<keycode>` (modifier-macro)
1. `<unicode-char>`
1. `unicode(<hex-digits>)`

`<expr>` has the form `<token1> <token2>...` where each token is one of:

* A valid keycode or action.
* A modifier-macro.
* A contiguous list of unicode characters.

`macro()` taps out the expression, `hold()` activates the keycodes on a key press and `release()`
deactivates the keycodes on key release. `macro(hold(<expr1>) release(<expr2>))` will activate
`<expr1>` on key press and deactivate `<expr2>` on key release.

`<unicode-char>` is a unicode character not in the basic keycode range; it is converted to
`unicode(<hex-digits>)` which will invoke the `global.unicode_prefix` action, type out the
hex-digits, and finally invoke the `global.unicode_suffix` action.

`<modifier-list>-<keycode>` is a list of modifier codes separated by a dash `-` (See [modifiers][2])
followed by any valid keycode. This will report the modifiers along with the keycode; for example
`S-1` will normally produce a bang `!`.

The following are all valid macro expressions:

* `C-d`
* `hold(C-a)`
* `A-S-backspace`
* `macro(He llo space delay(500) 🌏)`

Splitting into smaller tokens serves as an escaping mechainism: `macro(space)` inserts a space,
`macro(sp ace)` writes "space".

### Modifier macros

Modifier macros report the given modifiers before reporting the `keycode`. The modifiers are
reported only when necessary. This makes them useful for keeping [layer modifiers][2] in place when
defining keys on that layer.[^note1]

#### Example

```ini
[nav:C]

j = left
u = C-left
```

Here when the `nav` layer is active holding `j` will result in a `leftcontrol` up then `left` down;
whereas holding `u` will result in just a `left` down.

---

[^note1]: Sometimes, when multiple modifiers are pressed, a modifier is released before the
    modifier-macro is pressed. This would result in the modifier-macro not sending the down
    event. To help prevent this modifier-macros double set the modifiers.

[1]: https://en.wikipedia.org/wiki/USB_human_interface_device_class
[2]: layers.md#modifiers
