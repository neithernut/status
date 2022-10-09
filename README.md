# Status line generator

This little utility prints the current date and time alongside some values
fetched from procfs, in regular intervals until it is terminated. It is meant
for window managers and such relying on external programs for status info.

This tool is meant purely for my own personal use. And it's only useful on
Linux as it depends on `io_uring` and looks for some things in `/proc` which
you'll probably not find on a BSD. Don't expect much development to happen
here, aside maybe from me changing what things are included in the status.


## Motivation

So I recently came around using [sway](https://swaywm.org/). Sway doesn't come
with a clock but the default config shipped (on some distros) does include a
snippet with a little shell-script calling `date` once a second. For awesome, I
already wrote a few widgets for displaying additional information such as system
load, battery status and core temperature. Naturally, I wanted more or less with
sway.

I could just have written a shell-script and be done with it, but I decided to
write a bit of C since it has been a while, and try out a few things:

### Update *on* the second

Typically, people write clock widgets just like any other thing updating
regularly: set up a timer firing once a second and wire it up to generate the
contents (i.e. the current time), or even more lazily `sleep(1)` in a loop.

While this works, you get the clock update anywhere in a second. Ideally, the
update would happen near the *beginning* of a second. Is this important for a
desktop widget? Probably (well, most certainly) not. But it's *nice* to have.

I'm actually not 100% sure whether my approach works, but worst case the thing
is on par with what you normally get.

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

Build using your favorite C compiler and libc supporting all the extensions
used by my code and `liburing`. You'll also have to link against `liburing`.


## License

This work is licensed under the MIT license. See the [LICENSE](./LICENSE) file
for details.

