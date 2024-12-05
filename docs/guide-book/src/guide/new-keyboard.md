# Creating a keyboard

While this guide won't help you build a physical keyboard it will help you how to create firmware for it.

## Initializing a firmware project

At present only the rp2040 microcontroller (MCU) is supported. RPK might work for other MCUs but this
guide is geared towords the rp2040. It may serve as a starting point; see the
[embassy][1] project for more help to configure the project files.

The `rpk-config init` command will create a new directory containing an skeleton project for you to
get started. Give it the name of the directory that you want to create:

```sh
rpk-config init my-first-keeb
```

It will ask a few questions before generating the project.
After answering the questions, you can change the current directory into the new project:

```sh
cd my-first-keeb
```

## Project Structure

Firmware is built from several files which define how to create a binary that can be flashed to the MCU:

```
my-first-keeb
|- .cargo
|  |- config.toml
|- src
|  |- main.rs
|- build.rs
|- Cargo.toml
|- memory.x
|- default-layout.rpk.conf
```

The `.cargo/config.toml` describes what platform you're on, and configures how to deploy your
keyboard.  `build.rs` and `src/main.rs` is the rust code to run the firmware; these should not need
any modification. `Cargo.toml` describes the rust packages needed to build the firmware. `memory.x`
describes the MCU memory layout. See the [Embassy Project Layout][2] and [RPK API][3] for more
details.

The `default-layout.rpk.conf` determines how pins on the MCU generate key-codes. This is described
in detail in [The Config File Firmware][4] section.

Once you have edited the above files appropriately you can flash the software to the keyboard using
the `cargo run` command. You may need to put the keyboard in to boot select mode first; this is done
on a Raspberry Pi by holding down the reset button before connecting to the usb port. Then run the
following command from within the project:

```sh
cargo run --release
```

[1]: https://embassy.dev/
[2]: https://embassy.dev/book/#_project_structure
[3]: https://crates.io/crates/rpk_builder
[4]: ../config-file/firmware.md
