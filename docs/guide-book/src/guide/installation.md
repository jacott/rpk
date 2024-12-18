# Installation

In order to build a custom keyboard firmware you need to install the `rpk-config` tool. This
requires that the rust programming language is installed (See [Install
Rust](https://www.rust-lang.org/tools/install)).


Once you have rust installed you can install the `rpk-config` tool.

Not strictly necessary but expected by the default settings are the packages: [`flip-link`][1] and
[`elf2uf2-rs`][2].

Install these packages using the following command:

```sh
cargo install rpk-config flip-link elf2uf2-rs
```

### Installing the latest master version

The version published to crates.io will sometimes be behind the version hosted on GitHub.  If you
need the latest version you can build it using:

```sh
cargo install --git https://github.com/jacott/rpk.git rpk-config
```

[1]: https://crates.io/crates/flip-link
[2]: https://crates.io/crates/elf2uf2-rs
