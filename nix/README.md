# Nix Flake for XC Bot

This directory contains Nix packaging for the XC Bot application.

## Structure

- `flake.nix` - Main flake definition with inputs and outputs
- `package.nix` - Rust package derivation
- `module.nix` - NixOS module for systemd service with secret management

## Usage

### Building the Package

```bash
# From the repository root
nix build ./nix

# Or from within nix/ directory
cd nix
nix build
```

### Using in NixOS Configuration

Add to your `flake.nix`:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    xc-bot.url = "github:dbrgn/xc-bot?dir=nix";
  };

  outputs = { self, nixpkgs, xc-bot }: {
    nixosConfigurations.myserver = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        xc-bot.nixosModules.default
        {
          services.xc-bot = {
            enable = true;
            threema = {
              gatewayId = "*MYGATEW";
              gatewaySecretFile = "/run/secrets/threema-gateway-secret";
              privateKeyFile = "/run/secrets/threema-private-key";
              adminId = "ADMIN123";
            };
          };
        }
      ];
    };
  };
}
```

### Configuration Options

#### Basic Options

- `services.xc-bot.enable` - Enable the service (default: `false`)
- `services.xc-bot.package` - The xc-bot package to use

#### Threema Options

- `services.xc-bot.threema.gatewayId` - Threema Gateway ID, must start with `*` (required)
- `services.xc-bot.threema.gatewaySecretFile` - Path to file containing the Gateway secret
  (required)
- `services.xc-bot.threema.privateKeyFile` - Path to file containing the hex-encoded private key
  (required)
- `services.xc-bot.threema.adminId` - Threema ID of the admin (default: `null`)

#### XContest Options

- `services.xc-bot.xcontest.intervalSeconds` - Query interval in seconds (default: `180`)

#### Server Options

- `services.xc-bot.server.listen` - HTTP server listen address in `host:port` format (default:
  `"127.0.0.1:3000"`)

#### Logging Options

- `services.xc-bot.logging.filter` - Log filter using tracing syntax (default:
  `"info,sqlx::query=warn"`)

### Example Configurations

#### Minimal

```nix
services.xc-bot = {
  enable = true;
  threema = {
    gatewayId = "*MYGATEW";
    gatewaySecretFile = "/run/secrets/threema-gateway-secret";
    privateKeyFile = "/run/secrets/threema-private-key";
  };
};
```

#### Full Configuration

```nix
services.xc-bot = {
  enable = true;

  threema = {
    gatewayId = "*MYGATEW";
    gatewaySecretFile = "/run/secrets/threema-gateway-secret";
    privateKeyFile = "/run/secrets/threema-private-key";
    adminId = "ADMIN123";
  };

  xcontest.intervalSeconds = 300;

  server.listen = "0.0.0.0:8080";

  logging.filter = "debug,sqlx::query=warn";
};
```

## Testing Locally

You can test the package build without NixOS:

```bash
# From repository root
nix build ./nix

# Or from within nix/ directory
cd nix
nix build

# Run checks
nix flake check
```

## Overlay

The NixOS module automatically applies the overlay, so `pkgs.xc-bot`
is available when using the module. If you need the overlay separately (e.g., for
use outside the module), you can apply it manually:

```nix
nixpkgs.overlays = [ xc-bot.overlays.default ];
```

## Development

To make changes to the Nix packaging:

1. Edit the relevant file (`package.nix`, `module.nix`, or `flake.nix`)
2. Test with `nix flake check`
3. Build with `nix build`
