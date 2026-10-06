# Rust SofaScore fetcher

Standalone replacement for the Go `sofascore_fetch` helper. It accepts one HTTPS SofaScore API URL, sends the legacy five request headers over a Chrome 149/macOS TLS and HTTP/2 profile, and writes successful response bytes to stdout. Errors go to stderr with exit status 1. Redirects are followed only when their target remains an allowed HTTPS `/api/v1/` URL.

The Chrome 149 profile was validated against the events endpoint used by the application; its 26-event response matched the former Go fetcher's 113,633 bytes exactly. Lineup and incident endpoints were also verified. The former Go helper used Chrome 152, which is not available in the pinned `wreq-util` release.

Build or test with Rust 1.88 or newer. The wreq TLS backend uses native dependencies; on Linux this requires a C/C++ toolchain, Perl, CMake, and libclang. On macOS, install the Xcode command line tools and CMake.

From the repository root, `bin/setup` builds and installs the binary. To build and install manually:

```sh
cargo build --release --locked \
  --manifest-path sofascore-fetch-rust/Cargo.toml \
  --target-dir sofascore-fetch-rust/target
cp sofascore-fetch-rust/target/release/sofascore_fetch bin/sofascore_fetch
chmod 0755 bin/sofascore_fetch
```

Run it with one API URL:

```sh
bin/sofascore_fetch 'https://www.sofascore.com/api/v1/unique-tournament/10783/season/89945/events/round/5'
```

Run the crate checks with:

```sh
cargo test --release --locked --manifest-path sofascore-fetch-rust/Cargo.toml
```
