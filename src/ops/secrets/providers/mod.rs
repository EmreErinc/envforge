pub mod akeyless;
pub mod aws_ssm;
pub mod azure;
pub mod bitwarden;
pub mod conjur;
pub mod doppler;
pub mod gcp;
pub mod infisical;
pub mod keeper;
pub mod onepassword;
pub mod pass;
pub mod sops;
pub mod vault;

use super::provider::ProviderRegistry;

pub use akeyless::AkeylessProvider;
pub use aws_ssm::AwsSsmProvider;
pub use azure::AzureKeyVaultProvider;
pub use bitwarden::BitwardenProvider;
pub use conjur::ConjurProvider;
pub use doppler::DopplerProvider;
pub use gcp::GcpSecretManagerProvider;
pub use infisical::InfisicalProvider;
pub use keeper::KeeperProvider;
pub use onepassword::OnePasswordProvider;
pub use pass::PassProvider;
pub use sops::SopsProvider;
pub use vault::VaultProvider;

/// Create a registry with all built-in providers.
pub fn create_default_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(VaultProvider));
    registry.register(Box::new(AwsSsmProvider));
    registry.register(Box::new(OnePasswordProvider));
    registry.register(Box::new(DopplerProvider));
    registry.register(Box::new(InfisicalProvider));
    registry.register(Box::new(GcpSecretManagerProvider));
    registry.register(Box::new(AzureKeyVaultProvider));
    registry.register(Box::new(BitwardenProvider));
    registry.register(Box::new(AkeylessProvider));
    registry.register(Box::new(ConjurProvider));
    registry.register(Box::new(SopsProvider));
    registry.register(Box::new(PassProvider));
    registry.register(Box::new(KeeperProvider));
    registry
}
