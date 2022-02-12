# abduction - gameboy emulator. because there sure aren't enough of these out there already!

(click for short video!)

[![zelda opening](http://img.youtube.com/vi/jRXfu4RTL3s/0.jpg)](http://www.youtube.com/watch?v=jRXfu4RTL3s "zelda opening")

this emulator was a fun 2-month project, but it's not being worked on anymore. do, however, mess around with it if you want :)

feature breakdown:
- cpu: passes all of blargg's tests.
- ppu: working scanline implementation. has some small bugs that need to be fixed, but gets dmg-acid2 right.
- apu: not implemented.
- memory: only `no mbc` and `mbc1` roms are supported.
- cgb: some sections of the code take cgb into account, but it's very far from being supported.


# using abduction

first of all, it's worth noting that you'll have to provide a game rom and a boot rom yourself when using abduction.

if you're on windows, download the latest version from the releases page and run with `abduction.exe --help` to learn more about using it. alternatively, or if you're not on windows, you can build abduction yourself.


# building abduction

to build abduction, clone the repo and do `cargo build --release`. optionally, also set your `RUSTFLAGS` environment variable to `-target-cpu=native` before building for better performance (theoretically).

abduction has only been tested on windows 10, but will very likely work just fine on linux and mac.


# tui debugger

there's an unfinished (and a little outdated regarding user experience) tui debugger in the source, under the `tdebugger` module. it's not compiled by default, but can be by turning on the feature of same name (by doing `cargo run --release --features tdebugger -- --help`, for example).