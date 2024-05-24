# zpm-prototype

This repository is a prototype of a Rust-powered package manager heavily inspired from Yarn Berry.

It's a work in progress and is not meant to be used in production.

## Differences in architecture

### Redesigned steps

The Berry codebase uses a fairly sequential architecture: resolution, then fetching, then linking. The zpm codebase, on the other hand, interlaces the resolution and the fetching. There are a few reasons for this:

- Various non-semver protocols require fetching to be able to resolve the dependencies (git dependencies, `file:` dependencies), so in practice even with separate steps we need a way to call one step from the other.

- Rust doesn't have great and efficient primitives to handle mutating a single store from multiple places (in practice we'd have to use `Arc<Mutex<Store>>` or something similar, but that kills some of the benefits of running the fetch in parallel).

- One of the goals of the project is to make commands as fast as we can. By interlacing the resolution and the fetching, we can start fetching the first package as soon as we know we need it, rather than waiting for the resolution to be done.

### Serialization protocol

Many types are using the `yarn_serialization_protocol` macro. String serialization required a lot of boilerplate to support Serde, TryFrom, and FromStr, and I wanted to have a single place where all those details were handled (with the idea that this would also allow us to standardize things like color management).

### JSON lockfile

The Berry lockfile was written in Yaml. Since performances are a heavy focus of zpm, I decided to switch to JSON for the lockfile. This allows us to use `serde_json` which is much faster than `serde_yaml`. Some improvements would be useful to decrease the risks of conflicts (namely by adding blank lines between each lockfile record), but it doesn't require Yaml.

### No plugins

This implementation doesn't currently support plugins. It's a significant departure from Berry, and I'm not sure whether it'll remain that way or not - it was implemented this way to make it easier to incrementally build this prototype, not because of an overarching design decision.
