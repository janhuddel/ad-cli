use comfy_table::{presets::UTF8_FULL, Table};

use crate::error::{AppError, Result};
use crate::ldap::user::UserRecord;

pub fn render_table(user: &UserRecord) {
    let mut table = Table::new();
    table.load_style(UTF8_FULL);
    table.set_header(vec!["Attribute", "Value"]);

    let rows: Vec<(&str, String)> = vec![
        (
            "sAMAccountName",
            user.sam_account_name.clone().unwrap_or_default(),
        ),
        (
            "userPrincipalName",
            user.user_principal_name.clone().unwrap_or_default(),
        ),
        (
            "Display Name",
            user.display_name.clone().unwrap_or_default(),
        ),
        ("Mail", user.mail.clone().unwrap_or_default()),
        ("Title", user.title.clone().unwrap_or_default()),
        ("Department", user.department.clone().unwrap_or_default()),
        (
            "Telephone",
            user.telephone_number.clone().unwrap_or_default(),
        ),
        ("Mobile", user.mobile.clone().unwrap_or_default()),
        (
            "Enabled",
            if user.enabled {
                "yes".into()
            } else {
                "no".into()
            },
        ),
        ("Flags", user.account_flags.join(", ")),
        (
            "Last Logon (this DC only, not replicated)",
            user.last_logon.clone().unwrap_or_else(|| "unknown".into()),
        ),
        (
            "Last Logon Timestamp (replicated, may lag ~14d)",
            user.last_logon_timestamp
                .clone()
                .unwrap_or_else(|| "unknown".into()),
        ),
        ("Created", user.when_created.clone().unwrap_or_default()),
        (
            "Account Expires",
            user.account_expires
                .clone()
                .unwrap_or_else(|| "never expires".into()),
        ),
        ("Distinguished Name", user.distinguished_name.clone()),
    ];
    for (k, v) in rows {
        table.add_row(vec![k.to_string(), v]);
    }
    println!("{table}");
}

pub fn render_json(user: &UserRecord) -> Result<()> {
    let text = serde_json::to_string_pretty(user).map_err(|e| AppError::Other(e.to_string()))?;
    println!("{text}");
    Ok(())
}
