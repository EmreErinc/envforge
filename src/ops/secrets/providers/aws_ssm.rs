use std::collections::HashMap;

use super::super::provider::{run_cli, SecretProvider, SecretsError};

pub struct AwsSsmProvider;

impl SecretProvider for AwsSsmProvider {
    fn name(&self) -> &str {
        "aws-ssm"
    }

    fn display_name(&self) -> &str {
        "AWS SSM Parameter Store"
    }

    fn binary_name(&self) -> &str {
        "aws"
    }

    fn install_hint(&self) -> &str {
        "https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html"
    }

    fn credential_fields(&self) -> Vec<&str> {
        // No strictly required fields — can use profile, IAM role, or explicit keys
        vec![]
    }

    fn optional_credential_fields(&self) -> Vec<&str> {
        vec!["access_key", "secret_key", "region", "profile"]
    }

    fn authenticate(&self, _credentials: &HashMap<String, String>) -> Result<(), SecretsError> {
        self.check_binary()?;
        // AWS CLI handles auth via env vars, profile, or IAM role — we just need the binary
        Ok(())
    }

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        let env_vars = build_env(credentials);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let mut args = vec![
            "ssm",
            "get-parameters-by-path",
            "--path",
            path,
            "--with-decryption",
            "--output",
            "json",
        ];

        let region_str;
        if let Some(region) = credentials.get("region") {
            region_str = region.clone();
            args.extend_from_slice(&["--region", &region_str]);
        }

        let output = run_cli("aws", &args, &env_refs, "aws-ssm")?;
        parse_ssm_output(&output, path)
    }

    fn push(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError> {
        let env_vars = build_env(credentials);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

        for (key, value) in secrets {
            let param_name = format!("{}/{}", path.trim_end_matches('/'), key);
            let mut args = vec![
                "ssm",
                "put-parameter",
                "--name",
                &param_name,
                "--value",
                value,
                "--type",
                "SecureString",
                "--overwrite",
            ];

            let region_str;
            if let Some(region) = credentials.get("region") {
                region_str = region.clone();
                args.extend_from_slice(&["--region", &region_str]);
            }

            run_cli("aws", &args, &env_refs, "aws-ssm")?;
        }

        Ok(secrets.len())
    }

    fn get(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        key: &str,
    ) -> Result<String, SecretsError> {
        let env_vars = build_env(credentials);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let param_name = format!("{}/{}", path.trim_end_matches('/'), key);
        let mut args = vec![
            "ssm",
            "get-parameter",
            "--name",
            &param_name,
            "--with-decryption",
            "--output",
            "json",
        ];

        let region_str;
        if let Some(region) = credentials.get("region") {
            region_str = region.clone();
            args.extend_from_slice(&["--region", &region_str]);
        }

        let output = run_cli("aws", &args, &env_refs, "aws-ssm")?;
        let json: serde_json::Value =
            serde_json::from_str(&output).map_err(|e| SecretsError::ParseError {
                provider: "aws-ssm".to_string(),
                message: e.to_string(),
            })?;

        json.get("Parameter")
            .and_then(|p| p.get("Value"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| SecretsError::ParseError {
                provider: "aws-ssm".to_string(),
                message: "unexpected JSON structure".to_string(),
            })
    }

    fn list(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<String>, SecretsError> {
        let secrets = self.pull(credentials, path)?;
        Ok(secrets.into_iter().map(|(k, _)| k).collect())
    }
}

fn build_env(credentials: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    let mut env = Vec::new();
    if let Some(key) = credentials.get("access_key") {
        env.push(("AWS_ACCESS_KEY_ID", key.clone()));
    }
    if let Some(secret) = credentials.get("secret_key") {
        env.push(("AWS_SECRET_ACCESS_KEY", secret.clone()));
    }
    if let Some(profile) = credentials.get("profile") {
        env.push(("AWS_PROFILE", profile.clone()));
    }
    if let Some(region) = credentials.get("region") {
        env.push(("AWS_DEFAULT_REGION", region.clone()));
    }
    env
}

fn parse_ssm_output(
    output: &str,
    path_prefix: &str,
) -> Result<Vec<(String, String)>, SecretsError> {
    let json: serde_json::Value =
        serde_json::from_str(output).map_err(|e| SecretsError::ParseError {
            provider: "aws-ssm".to_string(),
            message: e.to_string(),
        })?;

    let parameters = json
        .get("Parameters")
        .and_then(|p| p.as_array())
        .ok_or_else(|| SecretsError::ParseError {
            provider: "aws-ssm".to_string(),
            message: "unexpected JSON structure".to_string(),
        })?;

    let prefix = format!("{}/", path_prefix.trim_end_matches('/'));
    let mut result: Vec<(String, String)> = parameters
        .iter()
        .filter_map(|p| {
            let name = p.get("Name")?.as_str()?;
            let value = p.get("Value")?.as_str()?;
            let key = name.strip_prefix(&prefix).unwrap_or(name);
            Some((key.to_string(), value.to_string()))
        })
        .collect();

    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}
