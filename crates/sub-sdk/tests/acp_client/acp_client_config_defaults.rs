use super::*;

#[test]
fn acp_client_config_defaults() {
    let config = AcpClientConfig::default();
    assert_eq!(config.client_name, "sub");
}
