use std::path::PathBuf;

use ldap_cli_core::account::{self, LoginInput, LoginPrompts};
use ldap_cli_core::Result;

use crate::cli::LoginArgs;
use crate::APP;

const PROMPTS: LoginPrompts = LoginPrompts {
    host: "AD server host",
    base_dn: "Base DN (e.g. DC=corp,DC=example,DC=com)",
    bind_identity: "Bind identity (UPN, DOMAIN\\user, or full DN)",
    allow_anonymous: false,
};

pub async fn run(args: LoginArgs, config_override: Option<&PathBuf>) -> Result<()> {
    let input = LoginInput {
        host: args.host,
        port: args.port,
        base_dn: args.base_dn,
        bind_identity: args.bind_identity,
        anonymous: false,
        password_stdin: args.password_stdin,
        insecure_skip_verify: args.insecure_skip_verify,
    };
    account::login(&APP, input, &PROMPTS, config_override).await
}
