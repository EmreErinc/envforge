use std::collections::HashMap;

use super::super::provider::{
    env_refs_from_env, run_cli, run_cli_with_tempfile, sort_secret_pairs, validate_secret_name,
    validate_secret_value, SecretProvider, SecretsError,
};

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

    fn minimum_version(&self) -> Option<&str> {
        Some("2.13.0")
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

    fn build_provider_env(
        &self,
        credentials: &HashMap<String, String>,
    ) -> Vec<(&'static str, String)> {
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

    fn pull(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
    ) -> Result<Vec<(String, String)>, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let region_str = credentials.get("region").cloned().unwrap_or_default();
        let has_region = credentials.contains_key("region");

        let mut all_results = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut args = vec![
                "ssm",
                "get-parameters-by-path",
                "--path",
                path,
                "--recursive",
                "--with-decryption",
                "--output",
                "json",
            ];

            if has_region {
                args.extend_from_slice(&["--region", &region_str]);
            }

            let token_str;
            if let Some(ref token) = next_token {
                token_str = token.clone();
                args.extend_from_slice(&["--next-token", &token_str]);
            }

            let output = run_cli("aws", &args, &env_refs, "aws-ssm")?;
            let mut page = parse_ssm_output(&output, path)?;
            all_results.append(&mut page);

            // Check for pagination token
            let json: serde_json::Value =
                serde_json::from_str(&output).unwrap_or(serde_json::Value::Null);
            match json.get("NextToken").and_then(|t| t.as_str()) {
                Some(token) => next_token = Some(token.to_string()),
                None => break,
            }
        }

        all_results.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(all_results)
    }

    fn push(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        secrets: &[(String, String)],
    ) -> Result<usize, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        for (key, value) in secrets {
            validate_secret_name(key)?;
            validate_secret_value(value)?;
            let param_name = format!("{}/{}", path.trim_end_matches('/'), key);
            let mut args = vec![
                "ssm",
                "put-parameter",
                "--name",
                &param_name,
                "--value",
                "fileb://__TEMP__",
                "--type",
                "SecureString",
                "--overwrite",
            ];

            let region_str;
            if let Some(region) = credentials.get("region") {
                region_str = region.clone();
                args.extend_from_slice(&["--region", &region_str]);
            }

            run_cli_with_tempfile("aws", &args, value, &env_refs, "aws-ssm")?;
        }

        Ok(secrets.len())
    }

    fn get(
        &self,
        credentials: &HashMap<String, String>,
        path: &str,
        key: &str,
    ) -> Result<String, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

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

pub fn build_env(credentials: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    let provider = AwsSsmProvider;
    provider.build_provider_env(credentials)
}

pub fn parse_ssm_output(
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

    sort_secret_pairs(&mut result);
    Ok(result)
}
