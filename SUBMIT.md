# Publishing Horizon Studio in Horizon's mod registry

Once it is in the registry, players install it from the launcher with one click (Mods, Horizon, Horizon Studio,
Install), and the launcher builds it into their game by itself. After that, script mods are drag and drop.

## What you need

- This repository, [vermingov/horizon-studio](https://github.com/vermingov/horizon-studio), **public**: the registry's `source` link must show the code the file was built from.
- `target/horizon_studio-0.1.0.hnmod`: `python3 tools/studio.py package` makes it (its SHA-256 is in
  `registry/horizon_studio.toml`; a rebuilt file of the same sources has the same contents).
- `registry/horizon_studio.png`: the icon (64x64).

## Submit

1. Go to [playhorizon.dev/mods/submit](https://playhorizon.dev/mods/submit) and sign in with GitHub.
2. Pick `horizon_studio-0.1.0.hnmod`.
3. Fill in the page from [`registry/horizon_studio.toml`](registry/horizon_studio.toml): name, summary, description,
   license (MIT), source (this repository's address) and the icon. Your GitHub account becomes its owner: only you
   publish its updates.
4. The form checks the file and opens a pull request in
   [horizon-rust/horizon-mods](https://github.com/horizon-rust/horizon-mods). Its checks run there; a reviewer reads
   the source and may ask for changes in the pull request. Merging publishes it.

What the registry's checks run, already run on this file (`cargo horizon-mod`, from Horizon's main):

| Check | Result |
|---|---|
| `cargo horizon-mod check horizon_studio-0.1.0.hnmod` | native-mod, `native-1/baa379d.../0d6b103...`, for 26.3 |
| `cargo horizon-mod registry review` | a new native-mod |
| `cargo horizon-mod registry check` | the file passes |
| `cargo horizon-mod registry build` | the index lists it |

Reviewers also expect the bundle to build on the pinned Horizon: `python3 tools/studio.py check` (this repository's
CI runs it on every push).

## Updates

Change the version in `mixins/horizon_studio/horizon-mixin.toml`, `compatibility.json` and `Cargo.toml`, run
`python3 tools/studio.py package`, and submit the new file through the same form: it adds a version to your entry.
A new Horizon Nightly that changes what the studio hooks needs a new version pinned to it (`horizon_revision`).
