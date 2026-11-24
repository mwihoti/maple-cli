use super::*;

#[test]
fn classifies_personal_access_tokens_by_prefix() {
    assert!(matches!(
        classify_maplecode_access_token("at-example"),
        MaplecodeAccessToken::PersonalAccessToken("at-example")
    ));
    assert!(matches!(
        classify_maplecode_access_token("header.payload.signature"),
        MaplecodeAccessToken::AgentIdentityJwt("header.payload.signature")
    ));
}
