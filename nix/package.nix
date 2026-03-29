{
  rustPlatform,
  bind,
  ...
}:
rustPlatform.buildRustPackage {
  pname = "xc-bot";
  version = "0.3.3";
  src = ../.;
  cargoLock.lockFile = ../Cargo.lock;
}
