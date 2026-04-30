Rewrite of https://github.com/vitaly-zdanevich/geeknote

By gpt-5.5 xhigh

## Build

Install the stable Rust toolchain, then build both binaries:

```sh
cargo build --release --bins
```

The binaries will be written to:

- `target/release/reeknote`
- `target/release/gnsync`

To build only one binary:

```sh
cargo build --release --bin reeknote
cargo build --release --bin gnsync
```

Run the local test suite with:

```sh
cargo test
```

## CI/CD

The GitLab CI pipeline in `.gitlab-ci.yml` builds release archives for:

- Linux x86_64
- Linux ARM64
- macOS

Each build uploads an artifact containing `reeknote` and `gnsync`.

The runner tags in `.gitlab-ci.yml` target GitLab.com hosted runners. If this project uses self-managed or differently tagged runners, adjust the `tags` and macOS `image` values.
