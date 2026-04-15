mod aws_ssm;
mod azure;
mod doppler;
mod gcp;
mod infisical;
mod onepassword;
mod vault;

use super::provider::ProviderRegistry;

pub use aws_ssm::AwsSsmProvider;
pub use azure::AzureKeyVaultProvider;
pub use doppler::DopplerProvider;
pub use gcp::GcpSecretManagerProvider;
pub use infisical::InfisicalProvider;
pub use onepassword::OnePasswordProvider;
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
    registry
}
