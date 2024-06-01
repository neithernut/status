# Status line generator

This little utility prints user configurable information such as the current
date and time or system information in regular intervals until it is terminated.
It is meant for window managers and such relying on external programs for status
info.

This tool is meant purely for my own personal use. And it's only useful on
Linux as it depends on `io_uring` and looks for some things in `/proc` which
you'll probably not find on a BSD. Don't expect much development to happen
here, aside maybe from me changing what things can be included in the status.


## Usage

    status [<specifier>...]

Each `specifier` being a specifier string of the form `<main>[:[<sub>,...]]`,
i.e. some "main" specifier, optionally followed by a comma-separated list of
sub-specifiers. The following "main" specifiers are recognised:

 * `datetime`, `time`, `dt`, `t`: the local date and time (with a fixed format).
   This specifier does not accept any sub-specifiers.
 * `load`, `l`: system load (as in 10min loadavg). This specifier does not
   accept any sub-specifiers.
 * `pressure`, `pres`, `psi`, `p`: resource pressure information (10min
   averages). The following sub-specifiers are accepted:
    * `cpu`, `c`: includes the CPU pressure indicator.
    * `memory`, `mem`, `m`: includes the memory pressure indicator.
    * `io`: includes the IO pressure indicator.
   If no sub-specifiers are provided, the status line will include `cpu`,
   `memory` and `io` in that order.
 * `memory`, `mem`, `m`: memory usage information. The following sub-specifiers
   are accepted:
   * `total`, `tot`, `t`: the total system memory (RAM)
   * `free`, `f`: unused system memory (RAM)
   * `available`, `availible`, `avail`, `a`: available system memory (RAM)
   * `totalswap`, `totsw`, `ts`: total swap
   * `freeswap`, `freesw`, `fs`: unused swap


## Background

The current version of `status` is a rewrite of a C program I wrote as a little
exercise, apparently in 2022. That was when I used [sway](https://swaywm.org/)
for some time. Sway doesn't come with a clock widget but the default config
shipped (on some distros) does include a snippet with a little shell-script
calling `date` once a second. For awesome, I already wrote a few widgets for
displaying additional information such as system load, battery status and core
temperature. Naturally, I wanted more or less with sway.

After writing the initial C version, I considered supporting more than just
time, load and pressure. However, the original, simple design proved to be too
inflexible. And because I intended to actually use it and extend it, I rewrote
it with a more flexible design. And I did so in Rust, both because it allows to
specify some abstractions in a more sensible way and because I'll actually use
this in "production", which means I do want it to adhere to some standard.

For the original C version back then, I had some objectives. And they still
stood for the re-write.

### Update *on* the second

Typically, people write clock widgets just like any other thing updating
regularly: set up a timer firing once a second and wire it up to generate the
contents (i.e. the current time), or even more lazily `sleep(1)` in a loop.

While this works, you get the clock update anywhere in a second. Ideally, the
update would happen near the *beginning* of a second. Is this important for a
desktop widget? Probably (well, most certainly) not. But it's *nice* to have.

I'm actually not 100% sure whether my approach works, especially in the event
of clock adjustments, which may occur rather regularly on machines running an
NTP client. At least multiple instances appear to "tick" in sync as one would
expect. Worst case the thing is on par with what you normally get.

### Fiddle around with `io_uring`

While I have made some experience with higher level async code, I did not yet
had the opportunity to work with `io_uring` directly. And to be honest, I was
lazy and used Jens Axboe's [liburing](https://github.com/axboe/liburing) which
provides some very useful abstractions.

Why would I do so? Well, I ultimately want to collect values from a bunch of
files in `/proc`. My old (predating `io_uring`!) widgets just opened the file
for the value they wanted whenever they needed the value. Yes, I didn't even
bother keeping the file open and `seek()`ing to the beginning. Mostly because
it's Lua, anyway.

Alternatively, we could fetch the file's contents in before. If we do so
*asynchronously*, we should use less time overall, since we don't continuously
waste time blocking on some `read()`. The fact that we (probably) spend less
time collecting the sources for our status line means the update hits the screen
even earlier in the second!


## Build

This thing can be built using `cargo`, the Rust build tool. It depends on crates
that are downloaded automatically during build, (see [Cargo.toml](./Cargo.toml)
for details), and outside of that the aforementioned liburing.


## License

This work is licensed under the MIT license. See the [LICENSE](./LICENSE) file
for details.

