# zpm-prototype

This repository is a prototype of a Rust-powered package manager heavily inspired from Yarn Berry.

It's a work in progress and is not meant to be used in production.

## Usage

1. Clone both this repository and the `berry` repository.

```bash
git clone https://github.com/yarnpkg/zpm.git
git clone https://github.com/yarnpkg/berry.git
```

2. Build the project. We build in release mode to reproduce as closely as possible the performances in which zpm will be used. Rust is known to be significantly slower in debug mode.

```bash
cd zpm && cargo build -r -p zpm-switch -p zpm
```

3. Run the tests.

```bash
cd zpm && ./yarn.sh berry test:integration
```

> [!NOTE]
> You can set the `BERRY_PATH` environment variable to a pre-existing clone of the `berry` repository to avoid cloning it again.

## Differences in architecture

### Redesigned steps

The Berry codebase uses a fairly sequential architecture: resolution, then fetching, then linking. The zpm codebase, on the other hand, interlaces the resolution and the fetching. There are a few reasons for this:

- Various non-semver protocols require fetching to be able to resolve the dependencies (git dependencies, `file:` dependencies), so in practice even with separate steps we need a way to call one step from the other.

- Rust doesn't have great and efficient primitives to handle mutating a single store from multiple places (in practice we'd have to use `Arc<Mutex<Store>>` or something similar, but that kills some of the benefits of running the fetch in parallel).

- One of the goals of the project is to make commands as fast as we can. By interlacing the resolution and the fetching, we can start fetching the first package as soon as we know we need it, rather than waiting for the resolution to be done.

### Serialization protocol

I wasn't satisfied with the `Display` and `Debug` traits, as they don't differentiate output intended for humans from output intended for serialization format (`Display` is arguably for humans, but `Debug` most certainly isn't intended for serialization).

> [!NOTE]
> I could have used the `Serialize` and `Deserialize` traits from `serde`, but if I remember correctly I was thinking that some data structures may want to be serialized / deserialized differently when targeting a file vs when targeting a string (typically a command-line argument).

To address that, I created three different traits:

- `ToHumanString` is meant to be used when printing things on the screen.
- `ToFileString` is meant to be used when writing something to a file.
- `FromFileString` is meant to be used when reading something from a file.

### JSON lockfile

The Berry lockfile was written in Yaml. Since performances are a heavy focus of zpm, I decided to switch to JSON for the lockfile. This allows us to use `sonic_rs` or `serde_json`, which are both much faster than `serde_yaml`.

Some improvements to the output format would be useful to decrease risks of conflicts when merging branches together, in particular by adding blank lines between each lockfile record, but we don't require Yaml for that.

### No plugins

This implementation doesn't currently support plugins. It's a significant departure from Berry, and I'm not sure whether it'll remain that way or not - it was implemented this way to make it easier to incrementally build this prototype, not because of an overarching design decision.
