# The Rust Programmable Keyboard Firmware Builder

RPK is a set of rust crates to build and configure hobbyist mechanical keyboard firmware. It
differentiates itself form other firmware builders---such as [QMK][1]---by the way the keyboard is
configured. Instead of fixed numbers of layers and macros with an assigned action to each row/column
RPK uses the configuration model of [keyd][2] which allows for many more layers and macros.

The current features of RPK include:

- Text file configuration which can be uploaded instantly via the `rpk-config` companion program (no
  need to re-flash firmware).
- 256 low cost layers (first 32 can be composite).
- 4096 macros.
- Sensible key overloading, oneshot layers and changeable base layout.
- Modifiers are layers.
- Mouse support with changeable acceleration profiles.
- n-key rollover, consumer and sys ctl keycode support.
- Unicode support.
- Ring file system for storing multiple configurations.
- Clear, reset, bootloader actions and reset on panic.
- Low latency debounce logic.
- Low overhead firmware - uses rust [embassy][3] async embedded framework.

[1]: https://docs.qmk.fm/
[2]: https://github.com/rvaiya/keyd
[3]: https://embassy.dev
