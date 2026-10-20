//! Ordinary Rust choices for Git; nixpkgs still supplies builders and dependencies.
use rusix::interop::raw::NixValue;

/// Perl-dependent features cannot be requested when Perl is disabled.
pub enum Perl {
    /// Let the package choose Perl for native builds and omit it for cross builds.
    PlatformDefault,

    /// Excludes Perl, SVN integration, and send-email together.
    Disabled,

    /// Enables Perl and independently chooses the integrations that require it.
    Enabled {
        /// Enables the Subversion bridge and its Perl dependencies.
        svn: bool,
        /// Enables send-email and its SMTP dependencies.
        send_email: bool,
    },
}

/// Rust-native package choices; platform-dependent defaults remain in Nix.
pub struct Git {
    /// Groups features constrained by upstream's Perl assertions.
    pub perl: Perl,
    /// Installs HTML documentation and the separate documentation output.
    pub manual: bool,
    /// Keeps Python support in Git's helper programs.
    pub python: bool,
    /// Selects PCRE2-backed regular expressions.
    pub pcre2: bool,
    /// Enables translations and gettext support.
    pub translations: bool,
    /// Installs the Tcl/Tk Git GUI programs.
    pub gui: bool,
    /// Installs the libsecret credential helper.
    pub libsecret: bool,
    /// Embeds the selected OpenSSH package in SSH invocation paths.
    pub ssh: bool,
    /// Overrides the Darwin-only credential helper default when supplied.
    pub keychain: Option<bool>,
    /// Overrides the platform-dependent install-check default when supplied.
    pub install_check: Option<bool>,
}

impl Git {
    /// The pinned package's ordinary defaults, without prematurely resolving a platform.
    pub fn defaults() -> Self {
        Self {
            perl: Perl::PlatformDefault,
            manual: true,
            python: true,
            pcre2: true,
            translations: true,
            gui: false,
            libsecret: false,
            ssh: false,
            keychain: None,
            install_check: None,
        }
    }

    /// Converts feature intent into explicit arguments; omitted choices keep Nix defaults.
    pub(super) fn arguments(self) -> Vec<(&'static str, NixValue)> {
        let mut fields = vec![
            ("withManual", self.manual.into()),
            ("pythonSupport", self.python.into()),
            ("withpcre2", self.pcre2.into()),
            ("nlsSupport", self.translations.into()),
            ("guiSupport", self.gui.into()),
            ("withLibsecret", self.libsecret.into()),
            ("withSsh", self.ssh.into()),
        ];

        match self.perl {
            Perl::PlatformDefault => {}
            Perl::Disabled => fields.extend([
                ("perlSupport", false.into()),
                ("svnSupport", false.into()),
                ("sendEmailSupport", false.into()),
            ]),
            Perl::Enabled { svn, send_email } => fields.extend([
                ("perlSupport", true.into()),
                ("svnSupport", svn.into()),
                ("sendEmailSupport", send_email.into()),
            ]),
        }

        fields.extend(self.keychain.map(|v| ("osxkeychainSupport", v.into())));
        fields.extend(self.install_check.map(|v| ("doInstallCheck", v.into())));

        fields
    }
}

/// A small usable Git without Perl-dependent helpers or documentation.
pub fn model() -> Git {
    Git {
        perl: Perl::Disabled,
        manual: false,
        python: false,
        pcre2: false,
        install_check: Some(false),
        ..Git::defaults()
    }
}
