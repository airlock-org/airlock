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
  adminClient = pkgs.python3.pkgs.qubes-core-admin-client;
  eventPython = pkgs.python3.withPackages (ps: [ ps.qubes-core-admin-client ]);
  eventListener = ../../runtime/tools/scripts/input-events.py;
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
    airlock.InputReceiverReady * @anyvm ${cfg.sender.target} deny
    EOF
  '';
  receiverReady = pkgs.writeShellScript "airlock-input-receiver-ready" ''
    set -eu

    [ "''${QREXEC_REMOTE_DOMAIN:-}" = dom0 ] || {
      echo "airlock.InputReceiverReady is restricted to dom0" >&2
      exit 1
    }
    [ -z "''${QREXEC_SERVICE_ARGUMENT:-}" ] || {
      echo "airlock.InputReceiverReady does not accept arguments" >&2
      exit 1
    }
    [ -c /dev/uinput ] && [ -w /dev/uinput ] || {
      echo "/dev/uinput is not ready" >&2
      exit 1
    }
    for handler in ${lib.concatMapStringsSep " " (service: "/etc/qubes-rpc/${service}") (
      builtins.attrValues services
    )}; do
      [ -x "$handler" ] || {
        echo "input receiver is unavailable: $handler" >&2
        exit 1
      }
    done
  '';
  connectInput = pkgs.writeShellScript "airlock-connect-gui-input" ''
    set -Eeuo pipefail

    ${lib.concatMapStringsSep "\n" (device: ''
      [ -e ${lib.escapeShellArg device.path} ] || {
        echo "physical input device is unavailable: ${device.path}" >&2
        exit 1
      }
    '') (builtins.attrValues cfg.sender.devices)}

    ${adminClient}/bin/qvm-check --running ${lib.escapeShellArg cfg.sender.target}
    xid_before="$(${adminClient}/bin/qvm-ls --raw-data --fields XID ${lib.escapeShellArg cfg.sender.target})"
    case "$xid_before" in
      ""|*[!0-9]*)
        echo "could not identify the running ${cfg.sender.target} instance" >&2
        exit 1
        ;;
    esac
    ${qrexec}/bin/qrexec-client -d ${lib.escapeShellArg cfg.sender.target} \
      -T -w 60 'DEFAULT:QUBESRPC airlock.InputReceiverReady dom0'
    # Reject a successful response from an instance that exited while the
    # readiness RPC was completing.
    ${adminClient}/bin/qvm-check --running ${lib.escapeShellArg cfg.sender.target}
    xid_after="$(${adminClient}/bin/qvm-ls --raw-data --fields XID ${lib.escapeShellArg cfg.sender.target})"
    [ "$xid_after" = "$xid_before" ] || {
      echo "${cfg.sender.target} restarted during input readiness" >&2
      exit 1
    }

    started=false
    cleanup() {
      if [ "$started" = true ]; then
        ${pkgs.systemd}/bin/systemctl stop airlock-gui-input.target
      fi
    }
    trap cleanup ERR
    ${pkgs.systemd}/bin/systemctl restart airlock-gui-input.target
    started=true
    ${lib.concatMapStringsSep "\n" (name: ''
      ${pkgs.systemd}/bin/systemctl is-active --quiet ${unitName name}.service
    '') (builtins.attrNames cfg.sender.devices)}
    trap - ERR
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
        ++ [
          {
            "qubes-rpc/airlock.InputReceiverReady" = {
              source = receiverReady;
              mode = "0555";
            };
            "qubes/rpc-config/airlock.InputReceiverReady" = {
              text = "force-user='root'\n";
              mode = "0444";
            };
          }
        ]
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

      # A Qubes event listener starts this target only after the already-started
      # GUI domain explicitly confirms that all receiver prerequisites exist.
      # It never starts or retries the VM itself.
      systemd.targets.airlock-gui-input = {
        description = "Forward selected physical input devices to the GUI domain";
        wants = map (name: "${unitName name}.service") (builtins.attrNames cfg.sender.devices);
      };
      systemd.services = {
        airlock-gui-input-connect = {
          description = "Connect physical input after the GUI domain is ready";
          after = [ "qubesd.service" ];
          serviceConfig = {
            Type = "oneshot";
            ExecStart = connectInput;
            TimeoutStartSec = 75;
          };
        };
        airlock-gui-input-events = {
          description = "Coordinate physical input from Qubes domain lifecycle events";
          wantedBy = [ "multi-user.target" ];
          requires = [ "airlock-setup-config.service" ];
          wants = [ "qubesd.service" ];
          after = [
            "airlock-setup-config.service"
            "qubesd.service"
          ];
          serviceConfig = {
            Type = "simple";
            ExecStart =
              "${eventPython}/bin/python ${eventListener}"
              + " --target ${lib.escapeShellArg cfg.sender.target}"
              + " --connect-unit airlock-gui-input-connect.service"
              + " --input-target airlock-gui-input.target"
              + " --systemctl ${pkgs.systemd}/bin/systemctl";
            Restart = "on-failure";
            RestartSec = 5;
          };
        };
      }
      // lib.mapAttrs' (
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
