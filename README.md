# yablocks

Yet another block based status bar generator.

yablocks is a tool for listening to various data sources and spitting out
formatted text suitable for use with text-based status bars like
dzen2, xmobar, or lemonbar.

## Why yet another?

I wanted a status bar generator that

- provided event based volume controls and network status,
- was status bar agnostic,
- gave good control of the output with flexible templating, and
- let me avoid running half a dozen slow, resource intensive shell scripts.

None of the existing status bar generators quite hit all the checkboxes.

## Installation

Check the releases on
[GitHub](https://github.com/julianandrews/yablocks/releases) for pre-built
binaries and `.deb` packages. If you have a working cargo/rust installation you
should also be able to build from source using `cargo build --release`.
yablocks works well as a standalone binary.

## Usage

yablocks waits for events and outputs text to `stdout`. For the most part,
you'll just want to pipe the output of yablocks to your status bar. For
instance:

    yablocks | dzen2

or:

    yablocks | lemonbar

or:

    yablocks | xmobar

When running lemonbar with clickable areas you may want to do something like:

    yablocks | lemonbar | sh

If launching xmobar from XMonad you can instead use yablocks with `CommandReader`:

    Config {
        commands = [
            Run CommandReader "/usr/bin/yablocks "yablocks"
        ],
        template = " %StdinReader% } { %yablocks% "
    }

## Configuration

You'll need to write a [toml](https://toml.io/en/) config file. You can put
this in the yablocks XDG config directory (by default
`~/.config/yablocks/config.toml`), or you can specify the config file path on
the command line.

A config file is a table of blocks along with a template referencing the block
names. See the examples
[here](https://github.com/julianandrews/yablocks/tree/master/examples).

### Testing Config

At its core yablocks is just a tool for spitting out template output to
`stdout` whenever data changes. Any error messages will output to `stderr`.

The easiest way to test your configuration is to simply run `yablocks` from the
command line and see what output you get.

## Blocks

Blocks have configurable inputs, and outputs which can be referenced in your
templates.

### command

Run a command and show output for each line.

#### Inputs

| name     | type         | description                                      |
| -------- | ------------ | ------------------------------------------------ |
| template | string       | template string (optional, default `{{output}}`) |
| command  | string       | command to run                                   |
| args     | list(string) | list of arguments to the command                 |

#### Outputs

| name     | type         | description                           |
| -------- | ------------ | ------------------------------------- |
| command  | string       | command provided                      |
| args     | list(string) | list of arguments provided            |
| output   | string       | last line of command output           |

### interval

Run a command periodically and show output.

#### Inputs

| name     | type          | description                                      |
| -------- | ------------- | ------------------------------------------------ |
| template | string        | template string (optional, default `{{output}}`) |
| command  | string        | command to run                                   |
| args     | array(string) | list of arguments to the command                 |
| interval | number        | how often to run the command in seconds          |

#### Outputs

| name     | type          | description                           |
| -------- | ------------- | ------------------------------------- |
| command  | string        | command provided                      |
| args     | array(string) | list of arguments provided            |
| interval | number        | interval provided                     |
| output   | string        | output of the last command invocation |

### inotify

Watch a file for changes and show content.

#### Inputs

| name     | type   | description                                        |
| -------- | ------ | -------------------------------------------------- |
| template | string | template string (optional, default `{{contents}}`) |
| file     | string | file to monitor                                    |

#### Outputs

| name     | type   | description          |
| -------- | ------ | -------------------- |
| file     | string | file to monitor      |
| contents | string | contents of the file |

Note: inotify is based on inodes. The inotify block will monitor the directory
containing the configured file so that if the file is deleted and recreated the
block will continue to function, but if the directory itself is deleted the
inode being watched will be gone, and changes won't be detected until the
directory is recreated and yablocks is restarted.

### network

Monitor status of a network device.

#### Inputs

| name     | type   | description                                         |
| -------- | ------ | --------------------------------------------------- |
| template | string | template string (optional, default `{{operstate}}`) |
| device   | string | network device to monitor (e.g. wlan0)              |

#### Ouputs

| name      | type    | description                            |
| --------  | ------- | -------------------------------------- |
| device    | string  | provided network device                |
| operstate | string  | state of the device                    |
| wireless  | boolean | whether the device is wireless         |
| essid     | string  | essid (if wireless and connected)      |
| quality   | number  | quality of wireless connection (0-100) |

### pulse-volume

Monitor a pulse audio sink.

#### Inputs

| name      | type   | description                                                      |
| --------- | ------ | ---------------------------------------------------------------- |
| template  | string | template string (optional, default `{{volume}}`)                 |
| sink-name | string | pulse audio sink to monitor (optional, defaults to default sink) |

#### Ouputs

| name      | type    | description               |
| --------  | ------- | ------------------------- |
| sink-name | string  | provided sink name        |
| volume    | number  | volume level              |
| muted     | boolean | whether the sink is muted |

### signal

Run a command whenever yablocks receives a signal. Signal number should be
between SIGRTMIN and SIGRTMAX (usually 34-64 inclusive for Linux).

#### Inputs

| name     | type          | description                                      |
| -------- | ------------- | -------------------------------------------------|
| template | string        | template string (optional, default `{{output}}`) |
| command  | string        | command to run                                   |
| args     | array(string) | list of arguments to the command                 |
| signal   | number        | RT signal to watch for                           |

#### Outputs

| name     | type          | description                           |
| -------- | ------------- | --------------------------------------|
| command  | string        | command provided                      |
| args     | array(string) | list of arguments provided            |
| signal   | number        | RT signal provided                    |
| output   | string        | output of the last command invocation |

## Contributing

Pull requests are welcome. For larger features or changes please open an issue
first to discuss your planned change.

PRs or suggestions for new blocks are welcome, but only if the functionality
isn't easily replicated using the existing blocks or if a custom block would
provide substantial performance benefits over an existing block combined with
a shell script.
