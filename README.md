# abduction - gameboy emulator. because there sure aren't enough of these out there already!

(click for short video!)

[![zelda opening](http://img.youtube.com/vi/jRXfu4RTL3s/0.jpg)](http://www.youtube.com/watch?v=jRXfu4RTL3s "zelda opening")

currently a WIP. progress breakdown:
- cpu: pretty much finished. passes all of blargg's tests.
- ppu: working scanline implementation. has some small bugs that need to be fixed, but gets dmg-acid2 right.
- apu: not started (yet™). next thing that's going to be worked on.
- memory: only `no mbc` and `mbc1` roms are currently supported, but support for other mbcs is a todo.
- sgb: not supported, and there are currently no plans to doing so.
- cgb: support planned. while work on it hasn't begun, some sections of the code take cgb into account.


# using abduction

first of all, it's worth noting that you'll have to provide a game rom and a boot rom yourself when using abduction.

if you're on windows, download the latest version from the releases page and run with `abduction.exe --help` to learn more about using it. alternatively, or if you're not on windows, you can build abduction yourself.


# building abduction

abduction requires nightly rust since it uses the [`mixed_integer_ops`](https://github.com/rust-lang/rust/issues/87840), [`trait_alias`](https://github.com/rust-lang/rust/issues/41517) and [`thread_is_running`](https://github.com/rust-lang/rust/issues/90470) unstable features. 

to install the latest nightly, do `rustup toolchain install nightly`. to use the nightly toolchain, either put `+nightly` after `cargo` when using cargo or do `rustup default nightly` to set it as the default toolchain.

to build abduction, clone the repo and do `cargo build --release`. optionally, also set your `RUSTFLAGS` environment variable to `-target-cpu=native` before building for better performance (theoretically).

abduction has only been tested on windows 10, but will very likely work just fine on linux and mac.


# tui debugger

there's an unfinished (and a little outdated regarding user experience) tui debugger in the source, under the `tdebugger` module. it's not compiled by default, but can be by turning on the feature of same name (by doing `cargo run --release --features tdebugger -- --help`, for example).