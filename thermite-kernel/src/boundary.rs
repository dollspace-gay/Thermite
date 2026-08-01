use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::capability::{CapabilityKind, Rights};
use crate::registry::{lookup, PlatformDomain, RegistryError, X86_64_PC_UEFI_SMP_V1};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryDeclaration<'a> {
    pub name: &'a str,
    pub signature: &'a str,
    pub contract: &'a str,
    pub domain: PlatformDomain,
    pub capability: Option<CapabilityKind>,
    pub rights: Rights,
    pub symbol: &'a str,
    pub source_contract_sha256: Option<&'a str>,
    pub source_reachable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosureError {
    Registry(RegistryError),
    DuplicateDeclaration,
    MissingReachableEntry,
    UnreachableFrozenEntry,
}

impl From<RegistryError> for ClosureError {
    fn from(value: RegistryError) -> Self {
        Self::Registry(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryInventory<'a> {
    names: Vec<&'a str>,
}

impl<'a> BoundaryInventory<'a> {
    pub fn close(
        declarations: &'a [BoundaryDeclaration<'a>],
        require_complete_profile: bool,
    ) -> Result<Self, ClosureError> {
        let mut names = BTreeSet::new();
        for declaration in declarations {
            if !names.insert(declaration.name) {
                return Err(ClosureError::DuplicateDeclaration);
            }
            let registered = lookup(declaration.name)?;
            if registered.signature != declaration.signature {
                return Err(ClosureError::Registry(RegistryError::SignatureDrift));
            }
            if registered.contract != declaration.contract {
                return Err(ClosureError::Registry(RegistryError::ContractDrift));
            }
            if registered.domain != declaration.domain {
                return Err(ClosureError::Registry(RegistryError::DomainDrift));
            }
            if registered.capability != declaration.capability {
                return Err(ClosureError::Registry(RegistryError::CapabilityDrift));
            }
            if registered.rights != declaration.rights {
                return Err(ClosureError::Registry(RegistryError::RightsDrift));
            }
            if registered.symbol != declaration.symbol {
                return Err(ClosureError::Registry(RegistryError::SymbolDrift));
            }
            if registered.source_contract_sha256 != declaration.source_contract_sha256 {
                return Err(ClosureError::Registry(RegistryError::SourceContractDrift));
            }
            if registered.source_reachable != declaration.source_reachable {
                return Err(ClosureError::Registry(RegistryError::ReachabilityDrift));
            }
        }
        if declarations.is_empty() {
            return Err(ClosureError::MissingReachableEntry);
        }
        if require_complete_profile
            && X86_64_PC_UEFI_SMP_V1
                .iter()
                .any(|entry| entry.source_reachable && !names.contains(entry.name()))
        {
            return Err(ClosureError::UnreachableFrozenEntry);
        }
        Ok(Self {
            names: names.into_iter().collect(),
        })
    }

    #[must_use]
    pub fn names(&self) -> &[&'a str] {
        &self.names
    }
}
