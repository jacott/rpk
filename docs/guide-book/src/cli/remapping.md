# Remapping A Keyboard

To remap a keyboard use the following command:

```sh
rpk-config upload <path-to-conf-file>
```

If the config file has a firmware section with a `vendor_id`, `product_id`, and/or `serial_number`
then `rpk-config` can find the corresponding keyboard automatically. If not you will need to supply
arguments to specify the keyboard if there is more than one. To see the list of connected keyboards
run the command: `rpk-config list-usb`.

Note: only devices with serial numbers starting with "rpk:" can be configured.

The config file will first be validated before being sent to the keyboard. You can validate the
config file without uploading by running the `rpk-config validate <path-to-conf-file>` command
instead.

Uploading will write a new config to the keyboard which will delete older files if the room is
needed. Uploading writes to a different location on the flash each time to preserve the life of the
flash.

Once a config is successfully writen to flash the keyboard will clear any active key presses and
macros then switch over to the new mapping. If the mapping is corrupt, the keyboard will fall back
to the defualt mapping supplied in the firmware. This usually all happens in under 20ms.
