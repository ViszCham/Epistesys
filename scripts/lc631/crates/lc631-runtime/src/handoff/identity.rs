//! Reported package pins are content labels, never authenticated package ingress.
#![forbid(unsafe_code)]

use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageComponent {
    Source,
    Binary,
    Plugin,
    Schema,
    Documentation,
}

const COMPONENTS: [PackageComponent; 5] = [
    PackageComponent::Source,
    PackageComponent::Binary,
    PackageComponent::Plugin,
    PackageComponent::Schema,
    PackageComponent::Documentation,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportedPackageIdentity {
    version: String,
    pins: [Digest32; 5],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    InvalidVersion,
    VersionMismatch,
    ComponentMismatch(PackageComponent),
}

/// Fixed component order: source, binary, plugin, schema, documentation.
impl ReportedPackageIdentity {
    pub fn new(version: &str, pins: [Digest32; 5]) -> Result<Self, IdentityError> {
        if version.is_empty()
            || version.len() > 64
            || !version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b))
        {
            return Err(IdentityError::InvalidVersion);
        }
        Ok(Self {
            version: version.to_owned(),
            pins,
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn pin(&self, component: PackageComponent) -> Digest32 {
        let index = match component {
            PackageComponent::Source => 0,
            PackageComponent::Binary => 1,
            PackageComponent::Plugin => 2,
            PackageComponent::Schema => 3,
            PackageComponent::Documentation => 4,
        };
        self.pins[index]
    }

    pub fn content_label(&self) -> Digest32 {
        let mut hash = Sha256::new();
        hash.update(b"epistesys-tlcx43-reported-package.v1\0");
        // Constructor bounds the version length to 64, so this conversion is exact.
        hash.update((self.version.len() as u32).to_le_bytes());
        hash.update(self.version.as_bytes());
        for pin in &self.pins {
            hash.update(pin.as_bytes());
        }
        Digest32::from_bytes(hash.finalize().into())
    }
}

/// Pairwise component equality only; does not authenticate the supplied pins.
pub fn compare_reported_identity(
    expected: &ReportedPackageIdentity,
    observed: &ReportedPackageIdentity,
) -> Result<(), IdentityError> {
    if expected.version != observed.version {
        return Err(IdentityError::VersionMismatch);
    }
    for (index, component) in COMPONENTS.into_iter().enumerate() {
        if expected.pins[index] != observed.pins[index] {
            return Err(IdentityError::ComponentMismatch(component));
        }
    }
    Ok(())
}
