# yablocks

Yet another block based status bar generator.

yablocks is a tool for listening to various data sources and spitting out
formatted text suitable for use with text-based status bars like
dzen2, xmobar, i3bar, or lemonbar.

## Why yet another?

So many status bar generators are focused around polling shell scripts. This is
slow, resource inefficient, and for latency sensitive displays like volume
meters, downright annoying.

I wanted a status bar generator that:

- was status bar agnostic,
- could output the exact markup I wanted with conditional formatting,
- let me avoid running half a dozen slow, resource intensive shell scripts, and
- made it easy to implement event based data sources.

Since it incorporates a fully featured templating engine
([Tera](https://tera.netlify.app/)), yablocks decouples data generation from
display. This means that the same block can be used to render the data with the
exact text and markup you want for for any status bar. This also means that you
often won't need to write and run wrapper scripts around a binary just to do
the formatting.

Where possible yablocks provides structured data to your templates rather than
raw text. All the built-in blocks will provide a mapping of relevant values,
and where possible yablocks supports parsing JSON inputs to provide the
specific data you need.

yablocks also goes out of its way to support event based inputs like inotify
based file watchers, signal watchers, and command readers.

You can use yablocks to run a shell script every 10 seconds if you want to, but
yablocks tries to make it so that you don't have to.

## Installation

Check the releases on
[GitHub](https://github.com/julianandrews/yablocks/releases) for pre-built
binaries and `.deb` packages. If you have a working cargo/rust installation you
should also be able to build from source using `cargo build --release`.
yablocks works well as a standalone binary.

## Usage

yablocks waits for events and outputs text to `stdout`. For the most part,
you'll just want to pipe the output of yablocks to your status bar.

### dzen2

You can pipe yablocks output to `dzen2`:

    yablocks | dzen2

### lemonbar

You can pipe yablocks output to `lemonbar`:

    yablocks | lemonbar

When running lemonbar with clickable areas you may want to do something like:

    yablocks | lemonbar | sh

### xmobar

You can pipe yablocks output to `xmobar`:

    yablocks | xmobar


If launching xmobar from XMonad you can instead use yablocks with `CommandReader`:

    # ~/.config/xmobar/xmobarrc
    Config {
        commands = [
            Run CommandReader "/usr/bin/yablocks "yablocks"
        ],
        template = " %StdinReader% } { %yablocks% "
    }

### i3bar

You can use yablocks as your `status_command`:

    # ~/.config/i3/config
    bar {
        status_command yablocks
    }

If using the i3bar [json protocol](https://i3wm.org/docs/i3bar-protocol.html)
you'll need to use the `header` field to send the version header. If you want
click event support you'll need to specify an `stdin-handler`. Something like:

    stdin-handler = { "command": "/path/to/my/command", "args": ["-a", "-b"] }

If using `stdin-handler`, `command` is required, but `args` is optional. All
click events will get passed on to the handler. See the
[docs](https://i3wm.org/docs/i3bar-protocol.html#_click_events) for details on
click events, and check out the
[examples](https://github.com/julianandrews/yablocks/tree/master/examples) to
see a simple handler.

## Configuration

You'll need to write a [toml](https://toml.io/en/) config file. yablocks will
look for it in your XDG config directory (by default
`~/.config/yablocks/config.toml`), or you can specify the config file path on
the command line.

To get started see the examples
[here](https://github.com/julianandrews/yablocks/tree/master/examples). For
compatibility I've kept fancy fonts out of the examples, but if your status bar
supports it, check out [Nerd Fonts](https://www.nerdfonts.com/) to add some
flair.

### Config file fields

- `template` - the main template to render
- `blocks` - a toml table of block configs
- `header` (optional) - an initial string to print on start
- `stdin-handler` (optional) - a command to run to process all stdin input

Only `template` and `blocks` are required. Both the main template and any
individual block templates use [Tera](https://tera.netlify.app/) as the
templating engine. Outputs from blocks can be used in their corresponding
templates. See the documentation below for available outputs.

The `header` and `stdin-handler` fields are primarily used for [i3bar](#i3bar).

### Testing Config

At its core yablocks is just a tool for spitting out templated output to
`stdout` whenever data changes. Any error messages will output to `stderr`.

The easiest way to test your configuration is to simply run `yablocks` from the
command line and see what output you get.

## Blocks

Blocks have inputs which can be provided in your config file, and outputs which
can be referenced in the block's template.

### command

Run a command and show output for each line.

If `json` is set to `true`, the each line will be parsed as JSON and all JSON
fields will be accessible from the `output` value in the outputs.

#### Inputs

| name     | type         | description                                                    |
| -------- | ------------ | -------------------------------------------------------------- |
| template | string       | template string (optional, default `{{output}}`)               |
| command  | string       | command to run                                                 |
| args     | list(string) | list of arguments to the command (optional, default `[]`)      |
| json     | boolean      | whether to interpret input as JSON (optional, default `false`) |

#### Outputs

| name     | type           | description                 |
| -------- | -------------- | --------------------------- |
| command  | string         | command provided            |
| args     | list(string)   | list of arguments provided  |
| output   | string or json | last line of command output |

### cpu

Monitor CPU usage.

The `cpu_times` map in the outputs includes `user`, `nice`, `system`, `idle`,
`iowait`, `irq`, `softirq`, `steal`, `guest`, `guest_nice`, and for convenience
`non_idle` which is the sum of everything except idle and iowait times, and is
probably what you actually want.

#### Inputs

| name     | type   | description                                                                        |
| -------- | ------ | ---------------------------------------------------------------------------------- |
| template | string | template string (optional, default `{{cpu_times.non_idle \| round(precision=1)}}`) |
| interval | number | how often to poll for CPU usage in seconds                                         |

#### Outputs

| name      | type        | description                     |
| --------- | ----------- | ------------------------------- |
| interval  | number      | interval provided               |
| cpu_times | map(number) | Map of CPU times as percentages |

### interval

Run a command periodically and show output.

If `json` is set to `true`, the output of command will be parsed as JSON and
all JSON fields will be accessible from the `output` value in the outputs.

#### Inputs

| name     | type         | description                                                    |
| -------- | ------------ | -------------------------------------------------------------- |
| template | string       | template string (optional, default `{{output}}`)               |
| command  | string       | command to run                                                 |
| args     | list(string) | list of arguments to the command (optional, default `[]`)      |
| interval | number       | how often to run the command in seconds                        |
| json     | boolean      | whether to interpret input as JSON (optional, default `false`) |

#### Outputs

| name     | type           | description                           |
| -------- | -------------- | ------------------------------------- |
| command  | string         | command provided                      |
| args     | list(string)   | list of arguments provided            |
| interval | number         | interval provided                     |
| output   | string or json | output of the last command invocation |

### inotify

Watch a file for changes and show content.

If `json` is set to `true`, the contents of the file will be parsed as JSON and
all JSON fields will be accessible from the `contents` value in the outputs.

Note: inotify is based on inodes. The inotify block will monitor the directory
containing the configured file so that if the file is deleted and recreated the
block will continue to function, but if the directory itself is deleted the
inode being watched will be gone, and changes won't be detected until the
directory is recreated and yablocks is restarted.

#### Inputs

| name     | type    | description                                                    |
| -------- | ------- | -------------------------------------------------------------- |
| template | string  | template string (optional, default `{{contents}}`)             |
| file     | string  | file to monitor                                                |
| json     | boolean | whether to interpret input as JSON (optional, default `false`) |

#### Outputs

| name     | type           | description          |
| -------- | -------------- | -------------------- |
| file     | string         | file to monitor      |
| contents | string or json | contents of the file |

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
| sink-name | string  | pulse audio sink name     |
| volume    | number  | volume level              |
| muted     | boolean | whether the sink is muted |

### signal

Run a command whenever yablocks receives a signal. Signal number should be
between SIGRTMIN and SIGRTMAX (usually 34-64 inclusive for Linux).

If `json` is set to `true`, the output of command will be parsed as JSON and
all JSON fields will be accessible from the `output` value in the outputs.

#### Inputs

| name     | type         | description                                                    |
| -------- | ------------ | -------------------------------------------------------------- |
| template | string       | template string (optional, default `{{output}}`)               |
| command  | string       | command to run                                                 |
| args     | list(string) | list of arguments to the command (optional, default `[]`)      |
| signal   | number       | RT signal to watch for                                         |
| json     | boolean      | whether to interpret input as JSON (optional, default `false`) |

#### Outputs

| name     | type           | description                           |
| -------- | -------------- | --------------------------------------|
| command  | string         | command provided                      |
| args     | list(string)   | list of arguments provided            |
| signal   | number         | RT signal provided                    |
| output   | string or json | output of the last command invocation |

### stdin

Read from stdin and show output for each line.

Stdin blocks can only be used if no `stdin_handler` has been configured.

If `json` is set to `true`, the output from stdin will be parsed as JSON and
all JSON fields will be accessible from the `output` value in the outputs.

#### Inputs

| name     | type    | description                                                    |
| -------- | ------- | -------------------------------------------------------------- |
| template | string  | template string (optional, default `{{output}}`)               |
| json     | boolean | whether to interpret input as JSON (optional, default `false`) |

#### Outputs

| name     | type           | description                 |
| -------- | -------------- | --------------------------- |
| output   | string or json | last line of command output |


## Contributing

Pull requests are welcome. For larger features or changes please open an issue
first to discuss your planned change.

PRs or suggestions for new blocks are welcome, but only if the functionality
isn't easily replicated using the existing blocks or if a custom block would
provide substantial performance benefits over an existing block combined with
a shell script.
