# Joku - A Roku CLI
Why not? If you spend a lot of time in your terminal, why not control your TV from there.

This is a wrapper around Roku's [External Control Protocol](https://developer.roku.com/docs/developer-program/dev-tools/external-control-api.md)

## Setup
Download the code however you prefer. 

You can build the binary with `cargo build` or you can install with `cargo install --path .`. The examples here assume the `cargo install` option.

```
# Download and install
git clone git@github.com:jasonmajors/joku.git && cd joku && cargo install --path .
```

We'll need a configuration file, `$HOME/.config/joku/config.toml`, to store the Roku device's IP and available applications.

You can create one by executing `joku discover` and following the prompt.

The `config.toml` file should look something like
```
[device]
name = "Some Roku TV"
addr = "<ip:port>"

[[apps]]
id = "551012"
type = "appl"
version = "13.5.56"
name = "Apple TV"
```

## Commands
## TODOs
