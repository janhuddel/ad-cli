use comfy_table::{presets::UTF8_FULL, Table};
use ldap_cli_core::{fuzzy_picker, stdout_is_interactive, Error, Result};
use serde::Serialize;

use crate::cli::RightsOutputFormat;
use crate::directory::UserEntry;

#[derive(Serialize)]
struct RightRecord<'a> {
    value: &'a str,
}

/// Same split as `ad groups`: an interactive fuzzy filter for a human at a
/// terminal, table/CSV/JSON for scripts (automatically whenever stdout isn't
/// a TTY).
pub fn display(
    user: &UserEntry,
    stage: &str,
    format: Option<RightsOutputFormat>,
    no_interactive: bool,
) -> Result<()> {
    let rights = user.rights();
    let want_interactive = format.is_none() && !no_interactive && stdout_is_interactive();

    match format {
        Some(RightsOutputFormat::Csv) => write_csv(&rights),
        Some(RightsOutputFormat::Json) => write_json(&rights),
        Some(RightsOutputFormat::Table) => {
            render_table(user, stage, &rights);
            Ok(())
        }
        None if want_interactive => interactive_filter(user, stage, &rights),
        None => {
            render_table(user, stage, &rights);
            Ok(())
        }
    }
}

fn user_label(user: &UserEntry) -> String {
    let name = user.display_name().unwrap_or_else(|| user.dn.clone());
    match user.first("cn") {
        Some(cn) if cn != name => format!("{name} ({cn})"),
        _ => name,
    }
}

fn render_table(user: &UserEntry, stage: &str, rights: &[String]) {
    let mut table = Table::new();
    table.load_style(UTF8_FULL);
    table.set_header(vec!["Right"]);
    for r in rights {
        table.add_row(vec![r.clone()]);
    }
    println!(
        "{} right(s) for {} in stage {stage}",
        rights.len(),
        user_label(user)
    );
    println!("{table}");
}

fn write_csv(rights: &[String]) -> Result<()> {
    let mut wtr = csv::Writer::from_writer(std::io::stdout());
    wtr.write_record(["value"])
        .map_err(|e| Error::Other(e.to_string()))?;
    for r in rights {
        wtr.write_record([r.as_str()])
            .map_err(|e| Error::Other(e.to_string()))?;
    }
    wtr.flush().map_err(Error::Io)?;
    Ok(())
}

fn write_json(rights: &[String]) -> Result<()> {
    let records: Vec<RightRecord> = rights.iter().map(|r| RightRecord { value: r }).collect();
    let text = serde_json::to_string_pretty(&records).map_err(|e| Error::Other(e.to_string()))?;
    println!("{text}");
    Ok(())
}

fn interactive_filter(user: &UserEntry, stage: &str, rights: &[String]) -> Result<()> {
    if rights.is_empty() {
        println!(
            "{} has no rights assigned in stage {stage}.",
            user_label(user)
        );
        return Ok(());
    }

    let prompt = format!(
        "{} right(s) for {} in stage {stage} — type to filter, Enter to show in full, Esc to quit",
        rights.len(),
        user_label(user)
    );
    while let Some(index) = fuzzy_picker::pick(&prompt, rights)? {
        println!("\n{}\n", rights[index]);
    }
    Ok(())
}
