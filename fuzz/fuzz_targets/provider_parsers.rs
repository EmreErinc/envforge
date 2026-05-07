#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(content) = std::str::from_utf8(data) {
        // Fuzz all provider JSON parsers with the same input
        let _ = envforge::ops::secrets::providers::vault::parse_kv_get_output(content);
        let _ = envforge::ops::secrets::providers::vault::parse_kv_list_output(content);
        let _ = envforge::ops::secrets::providers::aws_ssm::parse_ssm_output(content, "/fuzz");
        let _ = envforge::ops::secrets::providers::conjur::parse_conjur_list(content, "fuzz");
        let _ = envforge::ops::secrets::providers::doppler::parse_doppler_output(content);
        let _ = envforge::ops::secrets::providers::gcp::parse_gcp_list_output(content);
        let _ = envforge::ops::secrets::providers::infisical::parse_infisical_output(content);
        let _ = envforge::ops::secrets::providers::bitwarden::parse_bitwarden_output(content);
        let _ = envforge::ops::secrets::providers::onepassword::parse_item_output(content);
        let _ = envforge::ops::secrets::providers::sops::parse_sops_output(content);
        let _ = envforge::ops::secrets::providers::keeper::parse_keeper_list(content);
        let _ = envforge::ops::secrets::providers::keeper::parse_keeper_record(content);
        let _ = envforge::ops::secrets::providers::akeyless::parse_akeyless_list(content);
        let _ = envforge::ops::secrets::providers::akeyless::parse_akeyless_value(content, "/fuzz/test");
    }
});
