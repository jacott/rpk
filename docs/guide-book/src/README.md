# The Rust Programmable Keyboard Firmware Builder

RPK is a set of rust crates to build and configure hobbyist mechanical keyboard firmware. It
differentiates itself form other firmware builders---such as [QMK][1]---by the way the keyboard is
configured. Instead of fixed numbers of layers and macros with an assigned action to each row/column
RPK uses the configuration model of [keyd][2] which allows for many more layers and macros.

The current features of RPK include:

{{#include ../../../README.md:features}}

[1]: https://docs.qmk.fm/
[2]: https://github.com/rvaiya/keyd
