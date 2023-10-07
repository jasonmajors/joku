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
```
USAGE:
    joku <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    back           
    device-info    
    discover       Discover Roku devices on the network
    down           
    help           Prints this message or the help of the given subcommand(s)
    home           
    launch         Launches a Roku app
    left           
    list-apps      
    mute           
    pause          
    play           
    power-off      
    right          
    search         Search using the Roku External Control Protocol Search API:
                   [`https://developer.roku.com/docs/developer-
                   program/debugging/external-control-api.md#search-examples`] TODO: This doesn't seem to work great. Especially the launch flag
    select         
    up             
    volume-down    
    volume-up      

```
## TODOs
* Support launching apps other than YouTube
