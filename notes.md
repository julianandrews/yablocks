# Blocks to add

- Command(read from output)
- Signal(runs command when specified signal received)
- PulseVolume
    - https://gist.github.com/jasonwhite/1df6ee4b5039358701d2
    - https://menno.io/posts/pulseaudio_monitoring/
- Network device
    - use netlink, or possibly udev
- CPU
- Date?
- Battery?
- backlight?
- disk use?

# Helpers notes

Handlebars allows registering helpers. For instance:

    Handlebars.registerHelper("color-wrap", function(options) {
      return `<fc=${options.hash.color}>${options.fn(this)}</fc>`;
    });

It might be useful to add some for:

- matches regex
- wrap in xml? specific bar color? allow generating wrappers in config by specifying prefix/suffix?
- collapse whitespace?

See
https://github.com/sunng87/handlebars-rust/blob/master/src/helpers/helper_extras.rs
for examples and to see what's already there.
