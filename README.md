# RPK - Rust Programmable Keyboard

RPK is a keyboard firmware builder written in rust for hobbyist mechanical keyboards. It is modeled
on the excellent configuration of [keyd][1]. Check out the [User Guide][2] for information about
building firmware for your keyboard.

# Features

<!-- ANCHOR: features -->
- Text file configuration which can be uploaded instantly via the `rpk-config` companion program (no
  need to re-flash firmware).
- 256 low cost layers (first 32 can be parts in composite layers).
- 4096 macros of arbitrary length.
- Tap dance (many actions on a single key).
- Sensible key overloading, oneshot layers and changeable base layout.
- Modifiers are layers.
- Mouse support with changeable acceleration profiles.
- n-key rollover, consumer and sys ctl keycode support.
- Unicode support.
- Ring file system for storing multiple configurations.
- Clear, reset, bootloader actions and reset on panic.
- Low latency debounce logic.
- Low overhead firmware - uses rust [embassy](https://embassy.dev) async embedded framework.
<!-- ANCHOR_END: features -->

# Example config file

Example is for a [3x3 macropad][4] with a [Raspberry Pi Pico][5] (rp2040) micro controller.

```ini
[firmware]

# USB interface
# =============
vendor_id           = 0xceeb # 4 hex-digit number; try to be unique.
product_id          = 0xb0ad # 4 hex-digit number; try to be unique per vendor_id.
serial_number       = rpk:1234 # must start with rpk: unique per product, vendor_id.
manufacturer        = Jacott
product             = RPK macropad
max_power           = 100 # 0 to 500 (mA). Default 100

# Chipset and pin assignments
# ===========================
chip                = rp2040
# Pin names are from the chipset crate. For rp2040 it is the embassy_rp crate
output_pins         = [PIN_4, PIN_5, PIN_6]
input_pins          = [PIN_7, PIN_8, PIN_9]
row_is_output       = true # the output pins are connected to the keyboard rows

# Memory allocation
# =================

# Flash ring file system used to hold layouts, security tokens, dynamic macros...
flash_size          = 2 * 1024 * 1024 # 2MB matches memory.x file
fs_base             = 0x100000 # room for other things like firmware
# ensure fs_size is multiple of flash erase size (4K)
fs_size             = flash_size - fs_base

# How much room to reserve for layout configuration + runtime requirements.
max_layout_size     = 8 * 1024

# How many messages can we queue to the usb interface without waiting.
report_buffer_size  = 32

# How many key events can we scan without waiting.
scanner_buffer_size = 32

[matrix:3x3]
# I/O pin mapping to key codes
# ============================
# Names for row,column and default main layer key code. Codes can be pseudo code like u1
# or k24 which will map to noop by default.

# 0x00 means start assigning from row 0, column 0. 0x5a would mean start from row 5,
# column 10. If there are more than 16 rows or columns you can use 4 hex digits like,
# say 0x1234 for row 18, column 52. A maximum of 127 rows and columns is allowed.

0x00                = 7 8 9
0x10                = 4 5 6
0x20                = 1 2 3

# Customized layers/layout
# ========================

[main] # main layout. Overrides the keycodes given in matrix section.

3                   = overload(mouse, 3) # mouse layer on hold, 3 on tap
1                   = overload(shift, 1) # shift layer on hold, 1 on tap

[mouse] # layers can have any name.

8                   =           mouseup
4                   = mouseleft mousedown mouseright
1                   = mouse1    mouse2

[shift] # control, shift, alt and gui (a.k.a super) correspond to the modifier keys

7                   = macro(hello space world) # taps out "hello world"

[shift+mouse] # composite mode actives when shift and mouse layers are active
# chanel mouse acceleration profiles
4                   = mouseaccel1 mouseaccel2 mouseaccel3
```

# Planned future features

- Web and binary Graphical configuration app.
- Host daemon app for displaying keyboard information and changing keyboard layers/configuration
  based on host application in focus.
- record/play macros and store in flash.
- Key chording.
- LED support.
- Bluetooth/RF/Wifi support.
- Multi mcu/split keyboard support.
- Macro support/Test on other mcus (other than rp2040).
- Templates for other mcus.
- Security features to protect uploading.
- Macros to load other keyboard mapping stored on keyboard.
- Hot load new firmware - (not needed for rp2040).


## License

MIT license ([LICENSE-MIT][6] or <http://opensource.org/licenses/MIT>)

[1]: https://github.com/rvaiya/keyd
[2]: https://jacott.github.io/rpk/
[4]: keyboards/rp2040/macropad-3x3/default-layout.rpk.conf
[5]: https://www.raspberrypi.com/products/raspberry-pi-pico/
[6]: LICENSE-MIT
