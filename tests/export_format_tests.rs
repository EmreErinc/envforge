//! Coverage for `ops::export_format::ExportFormat` name parsing and extension
//! mapping, including aliases, case-insensitivity, and unknown-format errors.

use envforge::ops::export_format::ExportFormat;

#[test]
fn test_export_format_parse_canonical_and_aliases() {
    assert_eq!(ExportFormat::parse("dotenv").unwrap(), ExportFormat::Dotenv);
    assert_eq!(ExportFormat::parse("env").unwrap(), ExportFormat::Dotenv);
    assert_eq!(ExportFormat::parse(".env").unwrap(), ExportFormat::Dotenv);
    assert_eq!(ExportFormat::parse("yml").unwrap(), ExportFormat::Yaml);
    assert_eq!(
        ExportFormat::parse("kubernetes").unwrap(),
        ExportFormat::K8s
    );
    assert_eq!(
        ExportFormat::parse("terraform").unwrap(),
        ExportFormat::Tfvars
    );
    assert_eq!(
        ExportFormat::parse("compose-secrets").unwrap(),
        ExportFormat::DockerSecrets
    );
}

#[test]
fn test_export_format_parse_is_case_insensitive() {
    assert_eq!(ExportFormat::parse("JSON").unwrap(), ExportFormat::Json);
    assert_eq!(ExportFormat::parse("YaMl").unwrap(), ExportFormat::Yaml);
    assert_eq!(ExportFormat::parse("Docker").unwrap(), ExportFormat::Docker);
}

#[test]
fn test_export_format_parse_unknown_errors() {
    let err = ExportFormat::parse("xml").unwrap_err();
    assert!(err.contains("Unknown format"));
    assert!(err.contains("xml"));
    assert!(ExportFormat::parse("").is_err());
}

#[test]
fn test_export_format_extension_mapping() {
    assert_eq!(ExportFormat::Dotenv.extension(), ".env");
    assert_eq!(ExportFormat::Docker.extension(), ".env"); // docker shares .env
    assert_eq!(ExportFormat::Json.extension(), ".json");
    assert_eq!(ExportFormat::Yaml.extension(), ".yaml");
    assert_eq!(ExportFormat::K8s.extension(), ".yaml"); // k8s emits yaml
    assert_eq!(ExportFormat::Toml.extension(), ".toml");
    assert_eq!(ExportFormat::Tfvars.extension(), ".tfvars");
    assert_eq!(ExportFormat::DockerSecrets.extension(), ""); // directory output
}
