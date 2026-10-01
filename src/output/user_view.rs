use chrono::{DateTime, Local, Utc};
use comfy_table::{presets::UTF8_FULL, Table};
use console::style;

use crate::error::{AppError, Result};
use crate::ldap::user::UserRecord;

const SEP: &str = "  ·  ";
const LABEL_WIDTH: usize = 7;

/// Default view: a borderless "card" that groups related attributes onto a
/// few lines and hides empty ones. Colors are dropped automatically by
/// `console` when stdout isn't a TTY (or `NO_COLOR` is set).
pub fn render_compact(user: &UserRecord) {
    print!("{}", compact_card(user, Utc::now()));
}

fn compact_card(user: &UserRecord, now: DateTime<Utc>) -> String {
    let mut out = String::new();

    let name = non_empty(&user.display_name)
        .or_else(|| non_empty(&user.sam_account_name))
        .unwrap_or(&user.distinguished_name);
    let role = join([non_empty(&user.title), non_empty(&user.department)], ", ");
    out.push_str(&style(name).bold().to_string());
    if !role.is_empty() {
        out.push_str(SEP);
        out.push_str(&role);
    }
    out.push_str("\n\n");

    let account = join(
        [
            non_empty(&user.sam_account_name),
            non_empty(&user.user_principal_name),
        ],
        SEP,
    );
    push_line(&mut out, "Account", &account);

    let mut status = vec![if user.enabled {
        style("● enabled").green().to_string()
    } else {
        style("● disabled").red().to_string()
    }];
    for flag in &user.account_flags {
        status.push(match flag.as_str() {
            "locked out" | "password expired" => style(flag).red().to_string(),
            _ => flag.clone(),
        });
    }
    push_line(&mut out, "Status", &status.join(SEP));

    let contact = join(
        [
            non_empty(&user.mail),
            non_empty(&user.telephone_number),
            non_empty(&user.mobile),
        ],
        SEP,
    );
    push_line(&mut out, "Contact", &contact);
    push_line(
        &mut out,
        "Office",
        non_empty(&user.physical_delivery_office_name).unwrap_or_default(),
    );

    push_line(
        &mut out,
        "Logon",
        &logon(user.last_logon.as_deref(), "this DC", now),
    );
    push_line(
        &mut out,
        "",
        &logon(user.last_logon_timestamp.as_deref(), "replicated", now),
    );

    let created = user
        .when_created
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|raw| parse_ts(raw).map_or_else(|| raw.to_string(), fmt_date));
    let expires = match user.account_expires.as_deref() {
        None | Some("never expires") => "never".to_string(),
        Some(raw) => match parse_ts(raw) {
            Some(dt) => {
                let text = format!("{} ({})", fmt_date(dt), relative(dt, now));
                if dt <= now {
                    style(text).red().to_string()
                } else if dt - now < chrono::Duration::days(14) {
                    style(text).yellow().to_string()
                } else {
                    text
                }
            }
            None => raw.to_string(),
        },
    };
    let mut dates = created.map(|c| vec![c]).unwrap_or_default();
    dates.push(format!("{} {expires}", style("Expires").dim()));
    push_line(&mut out, "Created", &dates.join(SEP));

    push_line(&mut out, "DN", &user.distinguished_name);
    out
}

fn push_line(out: &mut String, label: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    let label = format!("{label:<LABEL_WIDTH$}");
    out.push_str(&format!("  {}  {value}\n", style(label).dim()));
}

fn non_empty(v: &Option<String>) -> Option<&str> {
    v.as_deref().filter(|s| !s.is_empty())
}

fn join<'a>(parts: impl IntoIterator<Item = Option<&'a str>>, sep: &str) -> String {
    parts.into_iter().flatten().collect::<Vec<_>>().join(sep)
}

fn logon(raw: Option<&str>, source: &str, now: DateTime<Utc>) -> String {
    match raw.and_then(parse_ts) {
        Some(dt) => format!(
            "{} ({}, {source})",
            dt.with_timezone(&Local).format("%Y-%m-%d %H:%M"),
            relative(dt, now)
        ),
        None => format!("{} ({source})", raw.unwrap_or("unknown")),
    }
}

fn parse_ts(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn fmt_date(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%Y-%m-%d").to_string()
}

/// Coarse human-readable distance, e.g. "3d ago" or "in 2mo".
fn relative(dt: DateTime<Utc>, now: DateTime<Utc>) -> String {
    const MIN: i64 = 60;
    const HOUR: i64 = 60 * MIN;
    const DAY: i64 = 24 * HOUR;
    let secs = (now - dt).num_seconds();
    let abs = secs.abs();
    let amount = match abs {
        a if a < MIN => return "just now".to_string(),
        a if a < HOUR => format!("{}m", a / MIN),
        a if a < DAY => format!("{}h", a / HOUR),
        a if a < 30 * DAY => format!("{}d", a / DAY),
        a if a < 365 * DAY => format!("{}mo", a / (30 * DAY)),
        a => format!("{}y", a / (365 * DAY)),
    };
    if secs < 0 {
        format!("in {amount}")
    } else {
        format!("{amount} ago")
    }
}

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
            "Office",
            user.physical_delivery_office_name
                .clone()
                .unwrap_or_default(),
        ),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        parse_ts("2026-10-01T12:00:00+00:00").unwrap()
    }

    fn sample() -> UserRecord {
        UserRecord {
            sam_account_name: Some("mmustermann".into()),
            user_principal_name: Some("max.mustermann@corp.example".into()),
            display_name: Some("Max Mustermann".into()),
            mail: Some("max.mustermann@corp.example".into()),
            title: Some("Senior Engineer".into()),
            department: Some("IT Infrastructure".into()),
            physical_delivery_office_name: Some("Berlin, Raum 4.12".into()),
            telephone_number: Some("+49 30 1234".into()),
            mobile: None,
            distinguished_name: "CN=Max Mustermann,OU=Users,DC=corp,DC=example".into(),
            enabled: true,
            account_flags: vec!["password never expires".into()],
            last_logon: Some("2026-09-30T08:14:00+00:00".into()),
            last_logon_timestamp: Some("never".into()),
            when_created: Some("2019-03-11T10:02:00+00:00".into()),
            account_expires: Some("never expires".into()),
        }
    }

    #[test]
    fn relative_distances() {
        let n = now();
        assert_eq!(relative(n, n), "just now");
        assert_eq!(
            relative(parse_ts("2026-10-01T11:15:00Z").unwrap(), n),
            "45m ago"
        );
        assert_eq!(
            relative(parse_ts("2026-09-24T12:00:00Z").unwrap(), n),
            "7d ago"
        );
        assert_eq!(
            relative(parse_ts("2019-03-11T10:02:00Z").unwrap(), n),
            "7y ago"
        );
        assert_eq!(
            relative(parse_ts("2026-12-01T12:00:00Z").unwrap(), n),
            "in 2mo"
        );
    }

    #[test]
    fn card_layout() {
        console::set_colors_enabled(false);
        let card = compact_card(&sample(), now());
        println!("{card}");
        let lines: Vec<&str> = card.lines().collect();
        assert_eq!(
            lines[0],
            "Max Mustermann  ·  Senior Engineer, IT Infrastructure"
        );
        assert_eq!(
            lines[2],
            "  Account  mmustermann  ·  max.mustermann@corp.example"
        );
        assert_eq!(lines[3], "  Status   ● enabled  ·  password never expires");
        assert_eq!(
            lines[4],
            "  Contact  max.mustermann@corp.example  ·  +49 30 1234"
        );
        assert_eq!(lines[5], "  Office   Berlin, Raum 4.12");
        assert!(
            lines[6].starts_with("  Logon    2026-09-30 ")
                && lines[6].ends_with("(1d ago, this DC)")
        );
        assert_eq!(lines[7], "           never (replicated)");
        assert!(lines[8].ends_with("  ·  Expires never"));
        assert_eq!(lines.len(), 10);
    }

    #[test]
    fn empty_fields_are_hidden() {
        console::set_colors_enabled(false);
        let user = UserRecord {
            mail: None,
            telephone_number: Some(String::new()),
            title: None,
            department: None,
            physical_delivery_office_name: None,
            ..sample()
        };
        let card = compact_card(&user, now());
        assert!(!card.contains("Contact"));
        assert!(!card.contains("Office"));
        assert!(card.starts_with("Max Mustermann\n\n"));
    }
}
