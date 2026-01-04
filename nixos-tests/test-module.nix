{
  pkgs,
  modules,
  ...
}: {
  name = "xc-bot module";
  nodes.machine = {pkgs, ...}: {
    imports = modules;

    # Config being tested
    services.xc-bot = {
      enable = true;
      threema = {
        gatewayId = "*ABCDEFG";
        gatewaySecretFile = pkgs.writeText "gateway-secret" "mysecret";
        privateKeyFile = pkgs.writeText "private-key" "privkey";
      };
      server = {
        listen = "127.0.0.1:3000";
      };
    };
  };

  testScript = ''
    machine.start(allow_reboot = True)

    machine.wait_for_unit("multi-user.target")
  '';
}
