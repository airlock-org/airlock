{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.airlock.inputProxy;
  proxy = pkgs.callPackage ../../runtime/tools/pkgs/qubes-input-proxy { };
  qrexec = pkgs.python3.pkgs.qubes-core-qrexec.dom0;
  services = {
    keyboard = "qubes.InputKeyboard";
    mouse = "qubes.InputMouse";
    tablet = "qubes.InputTablet";
  };
  unitName = name: "airlock-gui-input-${name}";
  senderPolicyPackage = pkgs.runCommand "airlock-gui-input-policy" { } ''
    install -d -m 0755 "$out/etc/qubes/policy.d"
    cat > "$out/etc/qubes/policy.d/40-airlock-gui-input.policy" <<'EOF'
    ${lib.concatMapStringsSep "\n" (service: "${service} * @anyvm ${cfg.sender.target} deny") (
      builtins.attrValues services
    )}
    EOF
  '';
in
{
  options.airlock.inputProxy = {
    receiver.enable = lib.mkEnableOption "dom0 input reception in the physical GUI domain";
    sender = {
      enable = lib.mkEnableOption "manual dom0 input forwarding to a physical GUI domain";
      target = lib.mkOption {
        type = lib.types.strMatching "[a-zA-Z][a-zA-Z0-9_-]*";
        default = "sys-gui";
        description = "Already-running GUI domain that receives physical input.";
      };
      devices = lib.mkOption {
        default = { };
        description = "Explicit physical input selection, supplied by the machine configuration.";
        type = lib.types.attrsOf (
          lib.types.submodule {
            options = {
              path = lib.mkOption {
                type = lib.types.strMatching "/dev/input/by-(id|path)/[a-zA-Z0-9_.:+-]+";
                description = "Stable evdev symlink; never a boot-dependent event number.";
              };
              type = lib.mkOption {
                type = lib.types.enum (builtins.attrNames services);
                description = "Upstream receiver filter; touchpads use tablet (absolute events).";
              };
            };
          }
        );
      };
    };
  };

  config = lib.mkMerge [
    (lib.mkIf cfg.receiver.enable {
      boot.kernelModules = [ "uinput" ];
      environment.systemPackages = [ proxy ];
      environment.etc = lib.mkMerge (
        lib.mapAttrsToList (_type: service: {
          "qubes-rpc/${service}".source = pkgs.writeShellScript service ''
            set -eu
            # These handlers create input devices as root, including at the
            # greeter. No application VM may inject events into the GUI domain.
            [ "''${QREXEC_REMOTE_DOMAIN:-}" = dom0 ] || exit 1
            exec ${proxy}/etc/qubes-rpc/${service}
          '';
          "qubes/rpc-config/${service}".text = "force-user='root'\n";
        }) services
      );
    })
    (lib.mkIf cfg.sender.enable {
      assertions = [
        {
          assertion = config.virtualisation.qubes.dom0.enable;
          message = "The physical input sender must run in Qubes dom0.";
        }
        {
          assertion = cfg.sender.target != "dom0" && !cfg.receiver.enable;
          message = "Physical input forwarding must cross from dom0 to a separate GUI domain.";
        }
        {
          assertion =
            cfg.sender.devices != { }
            && lib.all (name: builtins.match "[a-zA-Z0-9_-]+" name != null) (
              builtins.attrNames cfg.sender.devices
            );
          message = "Input forwarding requires named devices with simple systemd-safe names.";
        }
        {
          assertion =
            let
              paths = map (device: device.path) (builtins.attrValues cfg.sender.devices);
            in
            builtins.length paths == builtins.length (lib.unique paths);
          message = "An input device may only be forwarded once.";
        }
      ];
      environment.systemPackages = [ proxy ];
      # Dom0's upstream qrexec-client administrative path does not consult
      # policy. Deny VM-originated input calls separately; the guest also
      # checks QREXEC_REMOTE_DOMAIN before opening uinput.
      # The Qubes module owns /etc/qubes/policy.d as one read-only joined
      # directory. Supply this policy through its package list rather than
      # attempting to create a child entry under that immutable directory.
      virtualisation.qubes.packages = [ senderPolicyPackage ];

      # Manual activation is intentional during the physical-display lab.
      # Starting this target grabs only the selected inputs. Stopping it, or
      # losing the qrexec connection, releases them back to dom0. Nothing
      # autostarts the GUI VM or retries a GPU assignment.
      systemd.targets.airlock-gui-input = {
        description = "Forward selected physical input devices to the GUI domain";
        wants = map (name: "${unitName name}.service") (builtins.attrNames cfg.sender.devices);
      };
      systemd.services = lib.mapAttrs' (
        name: device:
        lib.nameValuePair (unitName name) {
          description = "Forward ${name} to ${cfg.sender.target} using Qubes input proxy";
          partOf = [ "airlock-gui-input.target" ];
          after = [ "qubesd.service" ];
          unitConfig.ConditionPathExists = device.path;
          serviceConfig = {
            Type = "exec";
            ExecStart =
              "${qrexec}/bin/qrexec-client -d ${cfg.sender.target}"
              + " -l '${proxy}/bin/input-proxy-sender ${device.path}'"
              + " 'DEFAULT:QUBESRPC ${services.${device.type}} dom0'";
            Restart = "no";
            KillMode = "control-group";
            TimeoutStopSec = 5;
            UMask = "0077";
            PrivateTmp = true;
          };
        }
      ) cfg.sender.devices;
    })
  ];
}
