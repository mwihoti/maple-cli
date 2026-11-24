const PERSONAL_ACCESS_TOKEN_PREFIX: &str = "at-";

pub(super) enum MaplecodeAccessToken<'a> {
    PersonalAccessToken(&'a str),
    AgentIdentityJwt(&'a str),
}

pub(super) fn classify_maplecode_access_token(access_token: &str) -> MaplecodeAccessToken<'_> {
    if access_token.starts_with(PERSONAL_ACCESS_TOKEN_PREFIX) {
        MaplecodeAccessToken::PersonalAccessToken(access_token)
    } else {
        MaplecodeAccessToken::AgentIdentityJwt(access_token)
    }
}

#[cfg(test)]
#[path = "access_token_tests.rs"]
mod tests;
