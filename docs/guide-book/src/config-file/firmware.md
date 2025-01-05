# Firmware Section

The firmware section is mostly used to build the keyboard firmware but can also be used to determine
which keyboard to reconfigure (see [Command Line Tool][1]). This is done using the the fields:
`vendor_id`, `product_id`, and `serial_number`.

Here is an example firmware section

```ini
{{#include ../../../../rpk-macros/test/default-layout.rpk.conf:firmware}}
```

The `flash_size` corresponds to the `memory.x` flash desription. Currently only `chip = rp2040` is
supported.

[1]: ../cli/
