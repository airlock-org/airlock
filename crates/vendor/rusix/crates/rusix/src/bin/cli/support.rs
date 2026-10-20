//! Fixture-only helpers, not Rusix API or a proposed service catalogue.
use rusix::{Config, IntoConfig};

pub struct OpenSsh {
    /// Settings contributed by this fixture-only SSH model.
    config: Config,
}

impl OpenSsh {
    #[track_caller]
    pub fn new() -> Self {
        Self {
            config: Config::new(),
        }
    }

    #[track_caller]
    pub fn enable(mut self, enabled: bool) -> Self {
        self.config = self.config.set_dynamic("services.openssh.enable", enabled);
        self
    }

    #[track_caller]
    pub fn ports(mut self, ports: Vec<u16>) -> Self {
        self.config = self.config.set_dynamic("services.openssh.ports", ports);
        self
    }
}

impl IntoConfig for OpenSsh {
    fn into_config(self) -> Config {
        self.config
    }
}

impl Default for OpenSsh {
    #[track_caller]
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ExistingModule {
    OpenSsh,
    NixosLabel,
}

impl ExistingModule {
    pub fn path(self) -> &'static str {
        match self {
            Self::OpenSsh => "nixos/modules/services/networking/ssh/sshd.nix",
            Self::NixosLabel => "nixos/modules/misc/label.nix",
        }
    }
}

impl From<ExistingModule> for String {
    fn from(module: ExistingModule) -> Self {
        module.path().into()
    }
}
