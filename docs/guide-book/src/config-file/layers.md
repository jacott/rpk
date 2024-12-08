# layers

Layers allow keyboard switches to execute more than one action or keycode. Multiple layers may be
active at any given time. At least one layer is always active and is known as the base layout. The
base layout defaults to `main` which is initially defined by the [matrix section][1].

Each layer contains a list of assignments which alter the [keycode/action][2] produced by a keyboard
switch matrix location. Each assignment is of the form:

```
<location> = <action-list>
```

Where action list is a space separated list of actions and/or keycodes.

#### Example

```ini
[matrix:3x3]

0x00 = a b c
0x10 = d e f
0x20 = g h i

[main]

g = overload(nav, g)

[nav]

b =       up
d = left down right
g = layer(shift) macro(hello) layer(control)
```

## Modifiers

Besides the `main` layer there are five other layers that are always defined: `control`, `shift`,
`alt`, `gui`, and `altgr`. These are the modifier layers and are bound to the modifier keycodes.
This means that when, say the left (or right) control key is held, the `control` layer will become
active. The same applies to `shift` and `gui`. `alt` relates to the `leftalt` keycode, `altgr`
refers to the `rightalt` keycode. These modifiers can be applied to any user defined layer in the
form of a layer suffix. This makes the layer behave like a modifier layer. The format of the suffix
is a follows:

```
"[" <layer-name>[:<modifier-list>] "]"
```

Where `<modifier-list>` has the form:

```
<modifier>[-<modifier>]...
```

and each modifier is one of:

* **C** - Left Control
* **S** - Left Shift
* **A** - Left Alt
* **G** - Letf GUI (Meta)
* **RC** - Right Control
* **RS** - Right Shift
* **RA** - Right Alt (AltGr)
* **RG** - Right GUI (Meta)

#### Example

```ini
[matrix:3x3]

0x00 = 7 8 9
0x10 = 4 5 6
0x20 = nav leftshift rightshift

[main] # No modifiers may be applied main

nav = layer(nav)

[nav:A-G]

8 =       up
4 = left down right

[shift] # implies the :S suffix

nav = space
```

When the `nav` key is held the `nav` layer becomes active. Also because it has modifiers the
`leftalt` and `leftgui` keycodes are sent to the host to report that they are held.

When the `7` key is tapped whilst the `nav` key is still held the keycode for 7 will be sent to the
host followed by a release of the 7 keycode; the modifiers remain active through out.

Now if `8` key is tapped, still whilst the `nav` key remains held, the host is sent a report
indicating that the `leftalt` and `leftgui` have been released followed by the press of the `up`
keycode, then the release of the `up` then finally by the reapplication of the `leftalt` and
`leftgui`.

Now if `nav` is finally released then the host will receive a release of `leftalt` and `leftgui` and
the `nav` layer will be deactivated.

Layers can be definied more than once in a conf file but only the first definition can contain
modifiers; any subsequent definition with modifiers will ignore the modifiers.

[1]: matrix.md
[2]: actions.md
