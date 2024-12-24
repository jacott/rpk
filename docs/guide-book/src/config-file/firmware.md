# Firmware Section

The firmware section is mostly used to build the keyboard firmware but can also be used to determine
which keyboard to reconfigure (see [Command Line Tool][1]). This is done using the the fields:
`vendor_id`, `product_id`, and `serial_number`.

Here is an example firmware section

```ini
[firmware]

# USB interface
# =============
vendor_id           = 0xceeb # 4 hex-digit number; try to be unique.
product_id          = 0xb0ad # 4 hex-digit number; try to be unique per vendor_id.
serial_number       = rpk:1234 # must start with rpk: unique per product, vendor_id.

# All of the following fields are only used when building the keyboard firmware binary.

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
max_layout_size     = 8 * 1024 # (8K)

# How many messages can we queue to the usb interface without waiting.
report_buffer_size  = 32

# How many key events can we scan without waiting.
scanner_buffer_size = 32
```

The `flash_size` corresponds to the `memory.x` flash desription. Currently only `chip = rp2040` is
supported.

[1]: ../cli/
