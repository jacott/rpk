# Config File

The config file contains most of the details for building and modifying a keyboard. It is based on
the [keyd][1] mapping config file format with a some missing features, some extra features and
configuration options needed for a mechanical keyboard. Notably the [firmware section][2] defines
most of the parameters needed to convert key switch presses in to sending [USB HID][3] messages to
the host computer. The config file can be used in two places:

1. as part of the [rust project][4] that builds the firmware;
2. as input to the `rpk-config` [command line tool][5] to instantly change the behavior of a running
   keyboard.

Configuration files loosely follow a [INI][6] style format consisting of headers of the form
`[section_name]` followed by a set of bindings.  Lines beginning with a hash `#` are ignored.

A valid config file must at least have a [matrix][7] section.

Special characters like brackets `[` can be escaped with a backslash `\[`.



[1]: https://github.com/rvaiya/keyd
[2]: ./firmware.md
[3]: https://en.wikipedia.org/wiki/USB_human_interface_device_class
[4]: ../guide/new-keyboard.md
[5]: ../cli/
[6]: https://en.wikipedia.org/wiki/INI_file
[7]: ./matrix.md
