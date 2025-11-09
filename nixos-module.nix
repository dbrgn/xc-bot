self: {
  config,
  pkgs,
  lib,
  ...
}:
with lib; let
  cfg = config.services.xc-bot;
in {
  # Define the options that can be set for this module
  options.services.xc-bot = {
    enable = mkEnableOption "xc-bot";
    package = mkPackageOption pkgs "xc-bot" {};

    # Threema configuration
    threema = mkOption {
      type = types.submodule {
        options = {
          gatewayId = mkOption {
            type = types.str;
            description = lib.mdDoc "The Threema Gateway ID (starts with a *)";
            example = "*EXAMPLE";
          };
          gatewaySecretFile = mkOption {
            type = types.path;
            description = lib.mdDoc "Path to file containing the Threema Gateway secret";
            example = "/run/secrets/threema-gateway-secret";
          };
          privateKeyFile = mkOption {
            type = types.path;
            description = lib.mdDoc "Path to file containing the hex-encoded private key";
            example = "/run/secrets/threema-private-key";
          };
          adminId = mkOption {
            type = types.nullOr types.str;
            default = null;
            description = lib.mdDoc "Identity of the admin";
            example = "ADMIN123";
          };
        };
      };
      description = lib.mdDoc "Threema Gateway configuration";
    };

    # XContest configuration
    xcontest = mkOption {
      type = types.submodule {
        options = {
          intervalSeconds = mkOption {
            type = types.nullOr types.int;
            default = 180;
            description = lib.mdDoc "The query interval in seconds";
            example = 300;
          };
        };
      };
      default = {};
      description = lib.mdDoc "XContest configuration";
    };

    # Server configuration
    server = mkOption {
      type = types.submodule {
        options = {
          listen = mkOption {
            type = types.str;
            default = "127.0.0.1:3000";
            description = lib.mdDoc "The HTTP server listening host:port string";
            example = "0.0.0.0:8080";
          };
        };
      };
      default = {};
      description = lib.mdDoc "Server configuration";
    };

    # Logging configuration
    logging = mkOption {
      type = types.submodule {
        options = {
          filter = mkOption {
            type = types.nullOr types.str;
            default = "info,sqlx::query=warn";
            description = lib.mdDoc "The log filter (tracing syntax)";
            example = "debug,sqlx::query=warn";
          };
        };
      };
      default = {};
      description = lib.mdDoc "Logging configuration";
    };
  };

  # Config if a user enabled this module
  config = mkIf cfg.enable {
    assertions = [
      {
        assertion = lib.hasPrefix "*" cfg.threema.gatewayId;
        message = "services.xc-bot.threema.gatewayId must start with '*'";
      }
      {
        assertion = cfg.xcontest.intervalSeconds == null || cfg.xcontest.intervalSeconds > 0;
        message = "services.xc-bot.xcontest.intervalSeconds must be positive";
      }
      {
        assertion = lib.match ".*:[0-9]+" cfg.server.listen != null;
        message = "services.xc-bot.server.listen must be in 'host:port' format";
      }
    ];

    nixpkgs.overlays = [self.overlays.default];

    # Generate the TOML config file with placeholders for secrets
    systemd.services.xc-bot = let
      # Build the config structure
      configData = {
        threema =
          {
            gateway_id = cfg.threema.gatewayId;
            gateway_secret = "@GATEWAY_SECRET@";
            private_key = "@PRIVATE_KEY@";
          }
          // optionalAttrs (cfg.threema.adminId != null) {
            admin_id = cfg.threema.adminId;
          };

        xcontest = optionalAttrs (cfg.xcontest.intervalSeconds != null) {
          interval_seconds = cfg.xcontest.intervalSeconds;
        };

        server = {
          listen = cfg.server.listen;
        };

        logging = optionalAttrs (cfg.logging.filter != null) {
          filter = cfg.logging.filter;
        };
      };

      # Generate TOML config file
      configFile = pkgs.writeText "xc-bot-config.toml" (
        generators.toTOML {} configData
      );

      # Create a script that substitutes secrets and runs xc-bot
      startScript = pkgs.writeShellScript "xc-bot-start" ''
        set -euo pipefail

        # Read secrets
        GATEWAY_SECRET=$(cat "$CREDENTIALS_DIRECTORY/threema-gateway-secret")
        PRIVATE_KEY=$(cat "$CREDENTIALS_DIRECTORY/threema-private-key")

        # Create runtime config with substituted secrets
        RUNTIME_CONFIG=$(mktemp)
        trap "rm -f $RUNTIME_CONFIG" EXIT

        sed -e "s|@GATEWAY_SECRET@|$GATEWAY_SECRET|g" \
            -e "s|@PRIVATE_KEY@|$PRIVATE_KEY|g" \
            ${configFile} > "$RUNTIME_CONFIG"

        # Run xc-bot with the runtime config
        exec ${cfg.package}/bin/xc-bot -c "$RUNTIME_CONFIG"
      '';
    in {
      description = "A chat bot that notifies about new paragliding cross-country flights published on XContest";
      wantedBy = ["multi-user.target"];
      wants = ["network-online.target"];
      after = ["network-online.target"];

      serviceConfig = {
        ExecStart = startScript;

        # Secrets
        LoadCredential = [
          "threema-gateway-secret:${cfg.threema.gatewaySecretFile}"
          "threema-private-key:${cfg.threema.privateKeyFile}"
        ];

        # User and state config
        DynamicUser = true;
        StateDirectory = "xc-bot";
        WorkingDirectory = "/var/lib/xc-bot";

        # Restart policy
        Restart = "on-failure";
        RestartSec = "30s";

        # Security hardening
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        NoNewPrivileges = true;
        PrivateDevices = true;
        PrivateTmp = true;
        ProtectClock = true;
        ProtectControlGroups = true;
        ProtectHome = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        ProtectSystem = "strict";
        ReadWritePaths = [];
        RestrictAddressFamilies = ["AF_INET" "AF_INET6"];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        SystemCallFilter = ["@system-service" "~@privileged"];
      };
    };
  };
}
