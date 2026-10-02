use std::collections::HashMap;

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime};
use console::style;
use ldap_cli_core::{Error, Result};

use crate::directory::{self as d, UserEntry, RIGHTS_ATTR};

const SEP: &str = "  ·  ";
const LABEL_WIDTH: usize = 8;
/// An end date this close (or past) is highlighted.
const WARN_DAYS: i64 = 30;

/// Default view: a borderless card like `ad user` with the attributes that
/// matter (see the card constants in `directory.rs`), hiding empty lines.
/// `all` appends every attribute the server returned, since the IdM schema
/// varies. `names` maps lowercased person IDs (manager, modifier) to names.
pub fn render_compact(user: &UserEntry, stage: &str, names: &HashMap<String, String>, all: bool) {
    print!(
        "{}",
        compact_card(user, stage, names, all, Local::now().date_naive())
    );
}

fn compact_card(
    user: &UserEntry,
    stage: &str,
    names: &HashMap<String, String>,
    all: bool,
    today: NaiveDate,
) -> String {
    let mut out = String::new();
    let id = get(user, "cn");
    let name = user.display_name().unwrap_or_else(|| user.dn.clone());
    out.push_str(&style(name).bold().to_string());
    if let Some(role) = get(user, d::ROLE_ATTR) {
        out.push_str(SEP);
        out.push_str(role);
    }
    out.push('\n');
    let sub = join([id, Some(&format!("stage {stage}"))], SEP);
    out.push_str(&style(sub).dim().to_string());
    out.push_str("\n\n");

    push_line(&mut out, "Status", &status(user));
    push_line(
        &mut out,
        "Contact",
        get(user, d::MAIL_ATTR).unwrap_or_default(),
    );
    let unit = join(
        [
            d::OU_ATTRS.iter().find_map(|a| get(user, a)),
            get(user, d::OU_NAME_ATTR),
        ],
        "  ",
    );
    let org = join([non_empty(&unit), get(user, d::LOCATION_ATTR)], SEP);
    push_line(&mut out, "Org", &org);
    let company = get(user, d::COMPANY_ATTR).map(|c| format!("company {c}"));
    let cost = join([get(user, d::COST_CENTER_ATTR), company.as_deref()], SEP);
    push_line(&mut out, "Cost ctr", &cost);
    let manager = get(user, d::MANAGER_ATTR).map(|m| person(m, names));
    push_line(&mut out, "Manager", manager.as_deref().unwrap_or_default());
    push_line(&mut out, "Employed", &employment(user, today));

    let rights = user.rights().len();
    if rights > 0 {
        let mut text = rights.to_string();
        if let Some(id) = id {
            text.push_str(&format!("  {}", style(format!("(idm rights {id})")).dim()));
        }
        push_line(&mut out, "Rights", &text);
    }

    let modified = get(user, d::MODIFY_TIME_ATTR).map(|raw| {
        NaiveDateTime::parse_from_str(raw, "%d.%m.%Y %H:%M:%S").map_or_else(
            |_| raw.to_string(),
            |t| t.format("%Y-%m-%d %H:%M").to_string(),
        )
    });
    let modifier = get(user, d::MODIFIER_ATTR).map(|m| format!("by {}", person(m, names)));
    let modified = join([modified.as_deref(), modifier.as_deref()], "  ");
    push_line(&mut out, "Modified", &modified);
    push_line(&mut out, "DN", &user.dn);

    if all {
        out.push('\n');
        out.push_str(&style("All attributes").bold().to_string());
        out.push('\n');
        out.push_str(&all_attributes(user));
    } else {
        let count = filled(user).count();
        let hint = match id {
            Some(id) => format!("{count} attributes in total — idm user {id} --all"),
            None => format!("{count} attributes in total — add --all to list them"),
        };
        out.push('\n');
        out.push_str(&style(hint).dim().to_string());
        out.push('\n');
    }
    out
}

fn status(user: &UserEntry) -> String {
    let mut parts = Vec::new();
    match get(user, d::LOGIN_DISABLED_ATTR) {
        Some(v) if v.eq_ignore_ascii_case("true") => {
            parts.push(style("● login disabled").red().to_string())
        }
        Some(_) => parts.push(style("● login enabled").green().to_string()),
        None => {}
    }
    let state = get(user, d::STATUS_ATTR);
    let eff =
        get(user, d::EFF_STATUS_ATTR).filter(|e| !state.is_some_and(|s| s.eq_ignore_ascii_case(e)));
    for s in [state, eff].into_iter().flatten() {
        parts.push(if s.eq_ignore_ascii_case("aktiv") {
            s.to_string()
        } else {
            style(s).red().to_string()
        });
    }
    if let Some(t) = get(user, d::EMPLOYEE_TYPE_ATTR) {
        parts.push(t.to_string());
    }
    if get(user, d::TRAINEE_ATTR).is_some_and(|v| v.eq_ignore_ascii_case("true")) {
        parts.push("trainee".to_string());
    }
    parts.join(SEP)
}

fn employment(user: &UserEntry, today: NaiveDate) -> String {
    let since = get(user, d::FIRST_DAY_ATTR).map(|raw| {
        format!(
            "since {}",
            parse_date(raw).map_or(raw.to_string(), fmt_date)
        )
    });
    let until = get(user, d::TERMINATION_ATTR).map(|raw| match parse_date(raw) {
        Some(date) if date.year() == 9999 => "no end date".to_string(),
        Some(date) => {
            let days = (date - today).num_days();
            let text = format!("until {} ({})", fmt_date(date), relative_days(days));
            if days < 0 {
                style(text).red().to_string()
            } else if days <= WARN_DAYS {
                style(text).yellow().to_string()
            } else {
                text
            }
        }
        None => format!("until {raw}"),
    });
    join([since.as_deref(), until.as_deref()], SEP)
}

/// "M100  Erika Muster", or just the ID if the name is unknown.
fn person(id: &str, names: &HashMap<String, String>) -> String {
    match names.get(&id.to_lowercase()) {
        Some(name) => format!("{id}  {name}"),
        None => id.to_string(),
    }
}

/// The IdM stores dates as German `dd.mm.yyyy` strings.
fn parse_date(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw.trim(), "%d.%m.%Y").ok()
}

fn fmt_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// Coarse distance in days, e.g. "in 27d" or "3mo ago".
fn relative_days(days: i64) -> String {
    let abs = days.abs();
    let amount = match abs {
        0 => return "today".to_string(),
        a if a < 30 => format!("{a}d"),
        a if a < 365 => format!("{}mo", a / 30),
        a => format!("{}y", a / 365),
    };
    if days > 0 {
        format!("in {amount}")
    } else {
        format!("{amount} ago")
    }
}

/// Every non-empty attribute, sorted case-insensitively, one value per
/// line; the rights are only counted here — `idm rights` lists them.
fn all_attributes(user: &UserEntry) -> String {
    let mut out = String::new();
    let mut attrs: Vec<(&String, &Vec<String>)> = filled(user).collect();
    attrs.sort_by_cached_key(|(k, _)| k.to_lowercase());
    let width = attrs
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0);

    for (key, values) in attrs {
        if key.eq_ignore_ascii_case(RIGHTS_ATTR) {
            let summary = format!("{} value(s) — run 'idm rights' to list them", values.len());
            push_attr(&mut out, key, width, &summary);
            continue;
        }
        for (i, value) in values.iter().enumerate() {
            push_attr(&mut out, if i == 0 { key } else { "" }, width, value);
        }
    }
    out
}

fn filled(user: &UserEntry) -> impl Iterator<Item = (&String, &Vec<String>)> {
    user.attributes
        .iter()
        .filter(|(_, v)| v.iter().any(|s| !s.is_empty()))
}

fn push_attr(out: &mut String, label: &str, width: usize, value: &str) {
    let padded = format!("{label:width$}");
    out.push_str(&format!("  {}  {value}\n", style(padded).dim()));
}

fn push_line(out: &mut String, label: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    let label = format!("{label:<LABEL_WIDTH$}");
    out.push_str(&format!("  {}  {value}\n", style(label).dim()));
}

/// First value of `attr`, if it's not blank.
fn get<'a>(user: &'a UserEntry, attr: &str) -> Option<&'a str> {
    user.first(attr).filter(|s| !s.trim().is_empty())
}

fn non_empty(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

fn join<'a>(parts: impl IntoIterator<Item = Option<&'a str>>, sep: &str) -> String {
    parts.into_iter().flatten().collect::<Vec<_>>().join(sep)
}

pub fn render_json(user: &UserEntry) -> Result<()> {
    let text = serde_json::to_string_pretty(user).map_err(|e| Error::Other(e.to_string()))?;
    println!("{text}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn entry(attrs: &[(&str, &[&str])]) -> UserEntry {
        UserEntry {
            dn: "cn=P1,ou=users,o=example".into(),
            attributes: attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect()))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 2).unwrap()
    }

    fn employee() -> UserEntry {
        entry(&[
            ("cn", &["P1"]),
            ("fullName", &["Max Müller"]),
            ("PNW-funktion", &["Anwendungsentwicklung"]),
            ("loginDisabled", &["FALSE"]),
            ("PNW-status", &["aktiv"]),
            ("PNW-eff-status", &["aktiv"]),
            ("employeeType", &["INTERNAL"]),
            ("PNW-azubi", &["false"]),
            ("mail", &["max@example.org"]),
            ("ou", &["ABC"]),
            ("departmentNumber", &["ABC"]),
            ("PNW-ou-name", &["Plattformen"]),
            ("l", &["Musterstadt"]),
            ("costCenter", &["CC-1234"]),
            ("company", &["02"]),
            ("managerWorkforceID", &["W9"]),
            ("DirXML-FirstWorkingDayStr", &["01.04.2002"]),
            ("DirXML-TerminationDateStr", &["31.12.9999"]),
            ("rightvalue", &["r1", "r2", "r2"]),
            ("PNW-modifytime", &["29.11.2021 09:17:10"]),
            ("PNW-modifiername", &["P1"]),
            ("objectClass", &["person", "top"]),
        ])
    }

    fn names() -> HashMap<String, String> {
        HashMap::from([
            ("w9".to_string(), "Erika Chef".to_string()),
            ("p1".to_string(), "Max Müller".to_string()),
        ])
    }

    #[test]
    fn card_shows_the_essentials() {
        console::set_colors_enabled(false);
        assert_eq!(
            compact_card(&employee(), "e0", &names(), false, today()),
            "Max Müller  ·  Anwendungsentwicklung\n\
             P1  ·  stage e0\n\n\
             \x20 Status    ● login enabled  ·  aktiv  ·  INTERNAL\n\
             \x20 Contact   max@example.org\n\
             \x20 Org       ABC  Plattformen  ·  Musterstadt\n\
             \x20 Cost ctr  CC-1234  ·  company 02\n\
             \x20 Manager   W9  Erika Chef\n\
             \x20 Employed  since 2002-04-01  ·  no end date\n\
             \x20 Rights    2  (idm rights P1)\n\
             \x20 Modified  2021-11-29 09:17  by P1  Max Müller\n\
             \x20 DN        cn=P1,ou=users,o=example\n\
             \n22 attributes in total — idm user P1 --all\n"
        );
    }

    #[test]
    fn card_skips_missing_lines_and_flags_end_date() {
        console::set_colors_enabled(false);
        let user = entry(&[
            ("cn", &["Z1"]),
            ("fullName", &["Ingest (E)"]),
            ("loginDisabled", &["TRUE"]),
            ("PNW-status", &["inaktiv"]),
            ("departmentNumber", &["ABC"]),
            ("managerWorkforceID", &["W9"]),
            ("DirXML-TerminationDateStr", &["29.10.2026"]),
        ]);
        assert_eq!(
            compact_card(&user, "e0", &HashMap::new(), false, today()),
            "Ingest (E)\n\
             Z1  ·  stage e0\n\n\
             \x20 Status    ● login disabled  ·  inaktiv\n\
             \x20 Org       ABC\n\
             \x20 Manager   W9\n\
             \x20 Employed  until 2026-10-29 (in 27d)\n\
             \x20 DN        cn=P1,ou=users,o=example\n\
             \n7 attributes in total — idm user Z1 --all\n"
        );
    }

    #[test]
    fn employment_handles_past_and_unparsable_dates() {
        console::set_colors_enabled(false);
        let past = entry(&[("DirXML-TerminationDateStr", &["30.06.2026"])]);
        assert_eq!(employment(&past, today()), "until 2026-06-30 (3mo ago)");
        let odd = entry(&[
            ("DirXML-FirstWorkingDayStr", &["2002-04-01"]),
            ("DirXML-TerminationDateStr", &["unbekannt"]),
        ]);
        assert_eq!(
            employment(&odd, today()),
            "since 2002-04-01  ·  until unbekannt"
        );
    }

    #[test]
    fn all_lists_every_attribute_aligned() {
        console::set_colors_enabled(false);
        let user = entry(&[
            ("cn", &["P1"]),
            ("DirXML-TerminationDateStr", &["31.12.9999"]),
            ("mail", &["a@x", "b@x"]),
            ("empty", &[""]),
            ("rightValue", &["r1", "r2"]),
        ]);
        let card = compact_card(&user, "e0", &HashMap::new(), true, today());
        let all = card.split_once("All attributes\n").unwrap().1;
        assert_eq!(
            all,
            "  cn                         P1\n\
             \x20 DirXML-TerminationDateStr  31.12.9999\n\
             \x20 mail                       a@x\n\
             \x20                            b@x\n\
             \x20 rightValue                 2 value(s) — run 'idm rights' to list them\n"
        );
        assert!(!card.contains("attributes in total"));
    }
}
