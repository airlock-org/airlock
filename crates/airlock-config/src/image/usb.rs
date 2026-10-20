//! Install prepared Qubes state into a sparse USB disk using upstream NixOS disk assembly.

use super::store;
use airlock_build::{Application, Workspace};
use rusix::{
    interop::{
        NixExpression, NixPath,
        raw::{NixRepresentation, NixValue},
    },
    nix_record, nix_text,
    nixos::{ContextualModule, ModuleBody, ModuleContext},
};

/// Configure the USB carrier and its prepared payload without adding payload objects to dom0's store.
pub(crate) fn module(workspace: &Workspace) -> NixValue {
    ContextualModule::with_arguments(["modulesPath", "airlockUsbPreparedStore"], |context| {
        let prepared: NixValue = context.argument("airlockUsbPreparedStore");
        let unified = store::unified(&context.pkgs, workspace, &context.config, prepared.clone());
        let settings = unified.bind(|unified| {
            let config = &context.config;
            let topology = NixValue::if_else(
                config.clone().select("specialisation").has_attr("usb-lab"),
                config
                    .clone()
                    .select("specialisation.usb-lab.configuration.airlock.qubes.topology"),
                config.clone().select("airlock.qubes.topology"),
            );
            let runtime = store::runtime(&context.pkgs, workspace, topology, unified.clone());

            runtime.bind(|runtime| {
                let image = disk(&context, workspace, &unified, &runtime);
                let definitions = nix_record! {
                    "image": nix_record! {
                        "baseName": "airlock-usb-dev",
                        "format": "raw",
                        "efiSupport": true,
                    },
                    "virtualisation": nix_record! {
                        "diskSize": 32768_i64,
                    },
                    "system": nix_record! {
                        "build": nix_record! {
                            "usbPreparedRuntime": runtime,
                            "image": context.lib.clone().select("mkForce").call(image),
                        },
                    },
                    "environment": nix_record! {
                        "etc": nix_record! {
                            "airlock/usb-dev": nix_record! {
                                "text": "airlock-usb-dev-v2\n",
                            },
                        },
                        "systemPackages": NixValue::list([Application::Usb
                            .build(&context.pkgs, workspace)
                            .as_expression()]),
                    },
                };

                context.lib.clone().select("mkMerge").call(NixValue::list([
                    definitions,
                    prepared_configuration(&context, prepared),
                ]))
            })
        });
        let modules: NixPath = context.argument("modulesPath");
        let option = context.lib.clone().select("mkOption").call(nix_record! {
            "type": context.lib.clone().select("types.str"),
            "default": "go-rwx",
            "description": "Mode adjustment applied while installing the prepared VM store.",
        });

        ModuleBody::new(settings)
            .key("airlock.image.usb")
            .import_value(
                modules
                    .join_relative("virtualisation/disk-image.nix")
                    .unwrap()
                    .as_expression(),
            )
            .import_value(nix_record! {
                "options": nix_record! {
                    "airlock": nix_record! {
                        "image": nix_record! {
                            "preparedStoreMode": option,
                        },
                    },
                },
            })
    })
    .into_expression()
}

/// Detach topology's VM path references while retaining every artifact actually consumed by dom0.
fn prepared_configuration(context: &ModuleContext, prepared: NixValue) -> NixValue {
    let topology = context
        .pkgs
        .clone()
        .select("runCommand")
        .call("airlock-usb-prepared-topology.json")
        .call(nix_record! {
            "__structuredAttrs": true,
            "unsafeDiscardReferences": nix_record! {
                "out": true,
            },
        })
        .call(nix_text!(
            r#"
            cp {topology} "$out"
        "#,
            topology = store::json(
                &context.pkgs,
                "airlock-usb-topology.json",
                context.config.clone().select("airlock.qubes.topology"),
            ),
        ));
    let kernels = NixValue::builtin("map")
        .call(NixValue::function(|kernel| kernel.select("source")))
        .call(
            NixValue::builtin("attrValues")
                .call(context.config.clone().select("airlock.qubes.kernels")),
        );

    nix_record! {
        "environment": nix_record! {
            "etc": nix_record! {
                "airlock/qubes-topology.json": nix_record! {
                    "text": context.lib.clone().select("mkForce").call(NixValue::null()),
                    "source": topology,
                },
            },
        },
        "system": nix_record! {
            "extraDependencies": NixValue::concat_lists([
                NixValue::list([
                    prepared.clone().select("descriptors"),
                    prepared.clone().select("validation"),
                ]),
                kernels,
            ]),
            "build": nix_record! {
                "usbPreparedGuestStore": prepared,
            },
        },
    }
}

/// Insert runtime seeding around upstream activation while guarding the expected assembly hooks.
fn disk(
    context: &ModuleContext,
    workspace: &Workspace,
    unified: &NixValue,
    runtime: &NixValue,
) -> NixValue {
    let native = Application::Dom0
        .build(&context.pkgs, workspace)
        .as_expression();
    let recovery = context
        .config
        .clone()
        .select("specialisation.usb-text-recovery.configuration.system.build.toplevel");
    let closure = context
        .pkgs
        .clone()
        .select("closureInfo")
        .call(nix_record! {
            "rootPaths": NixValue::list([recovery.clone()]),
        });
    let mode = context.lib.clone().select("escapeShellArg").call(
        context
            .config
            .clone()
            .select("airlock.image.preparedStoreMode"),
    );
    let install: NixValue = nix_text!(
        r#"
        echo "Installing prepared Qubes runtime state"
        if test -e /mnt/var/lib/qubes/qubes.xml; then
          echo "Refusing to replace an existing Qubes database during image preparation" >&2
          exit 1
        fi

        # Reconstruct sparse allocation even when a substituted seed contains dense files.
        mkdir -p /mnt/var/lib/airlock/closure-store
        cp -a --sparse=always {unified}/pool/. /mnt/var/lib/airlock/closure-store/
        chmod -R {mode} /mnt/var/lib/airlock/closure-store
        mkdir -p /mnt/var/lib/qubes
        cp -a --sparse=always {runtime}/qubes/. /mnt/var/lib/qubes/
        cp -a {runtime}/pool-selections/. /mnt/var/lib/airlock/closure-store/
        chmod -R u+rwX /mnt/var/lib/qubes
        cp {runtime}/receipt.json /mnt/var/lib/airlock/prepared-runtime.json
    "#,
        unified = unified.clone(),
        mode = mode,
        runtime = runtime.clone(),
    )
    .into();
    let ownership: NixValue = nix_text!(
        r#"
        # Activation must create the installed qubes group before resolving its GID.
        nixos-enter --root /mnt -- chown -R root:qubes /var/lib/qubes
        chmod 0660 /mnt/var/lib/qubes/qubes.xml
        {native}/bin/airlock-dom0-service storage image install-host /mnt {closure}/store-paths {recovery}
    "#,
        native = native,
        closure = closure,
        recovery = recovery,
    )
    .into();
    let marker = context
        .pkgs
        .clone()
        .select("writeText")
        .call("airlock-usb-image")
        .call("airlock-usb-dev-v2\n");
    let image = disk_builder(context).call(disk_arguments(context, "airlock-usb").merge_attrs(
        nix_record! {
            "copyChannel": false,
            "contents": NixValue::list([nix_record! {
                "source": marker,
                "target": "/airlock-usb-image",
            }]),
        },
    ));

    context
        .lib
        .clone()
        .select("overrideDerivation")
        .call(image)
        .call(NixValue::function(|old| {
            let command = old.clone().select("buildCommand");
            let mounted = "mount $rootDisk $mountPoint";
            let unmount = "umount -R /mnt";
            let valid = context
                .lib
                .clone()
                .select("hasInfix")
                .call(mounted)
                .call(command.clone())
                .and(
                    context
                        .lib
                        .clone()
                        .select("hasInfix")
                        .call(unmount)
                        .call(command.clone()),
                );
            let replaced = NixValue::builtin("replaceStrings")
                .call(NixValue::list([
                    mounted.into(),
                    unmount.into(),
                    "-- /nix/var/nix/profiles/system/bin/switch-to-configuration boot".into(),
                ]))
                .call(NixValue::list([
                    NixValue::concat_text([mounted.into(), "\n".into(), install]),
                    NixValue::concat_text([ownership, "\n".into(), unmount.into()]),
                    // NIX_CONFIG overrides the installed daemon setting during chroot activation.
                    "-- env NIX_REMOTE=local NIX_CONFIG='store = local' \
                     /nix/var/nix/profiles/system/bin/switch-to-configuration boot"
                        .into(),
                ]))
                .call(command);
            let definitions = nix_record! {
                "buildCommand": replaced,
                "preVM": NixValue::concat_text([
                    nix_text!(
                        r#"
                            echo "Validating prepared VM store before image assembly"
                            {check}
                        "#,
                        check = unified.clone().select("checkCommand"),
                    ).into(),
                    old.select("preVM"),
                ]),
            };

            NixValue::if_else(
                valid,
                definitions,
                NixValue::builtin("throw").call(
                    "Upstream disk assembly no longer exposes the required mount and unmount hooks."
                ),
            )
        }))
}

/// Import upstream disk assembly from the same NixOS module tree as the evaluated host.
pub(super) fn disk_builder(context: &ModuleContext) -> NixValue {
    NixValue::builtin("import").call(nix_text!(
        "{modules}/../lib/make-disk-image.nix",
        modules = context.argument::<NixValue>("modulesPath"),
    ))
}

/// Reserve EFI capacity for retained Xen/kernel/initrd generations and install staging.
pub(super) fn disk_arguments(context: &ModuleContext, label: &str) -> NixValue {
    nix_record! {
        "config": context.config.clone(),
        "lib": context.lib.clone(),
        "pkgs": context.pkgs.clone(),
        "baseName": context.config.clone().select("image.baseName"),
        "format": context.config.clone().select("image.format"),
        "diskSize": context.config.clone().select("virtualisation.diskSize"),
        "partitionTableType": "efi",
        "bootSize": "3G",
        "label": label,
    }
}
