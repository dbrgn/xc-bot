# Agent notes

## Rust toolchain

The Rust toolchain version pinned in `.github/workflows/ci.yml` (`RUST_TOOLCHAIN`) and in the
`Dockerfile` must match the default Rust version of the NixOS release used in `nix/flake.nix`
(e.g. Rust 1.95 for nixos-26.05). The nix package is built with the Rust compiler from nixpkgs, so
code and dependencies must not require a newer Rust version than that.

When bumping the NixOS release, bump the Rust toolchain accordingly (and vice versa: only use a
newer toolchain once a NixOS release ships it).
