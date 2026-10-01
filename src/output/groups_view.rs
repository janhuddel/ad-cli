use comfy_table::{presets::UTF8_FULL, Table};
use dialoguer::FuzzySelect;

use crate::cli::GroupsOutputFormat;
use crate::error::{AppError, Result};
use crate::ldap::groups::GroupRecord;
use crate::output::stdout_is_interactive;

/// Primary UX is the interactive fuzzy filter (for a human at a terminal who
/// needs to make sense of 100+ groups); CSV/JSON/plain-table are the
/// scriptable fallbacks, used automatically whenever stdout isn't a TTY.
pub fn display(
    groups: &[GroupRecord],
    format: Option<GroupsOutputFormat>,
    no_interactive: bool,
) -> Result<()> {
    let want_interactive = format.is_none() && !no_interactive && stdout_is_interactive();

    match format {
        Some(GroupsOutputFormat::Csv) => write_csv(groups),
        Some(GroupsOutputFormat::Json) => write_json(groups),
        Some(GroupsOutputFormat::Table) => {
            render_table(groups);
            Ok(())
        }
        None if want_interactive => interactive_filter(groups),
        None => {
            render_table(groups);
            Ok(())
        }
    }
}

fn render_table(groups: &[GroupRecord]) {
    let mut table = Table::new();
    table.load_style(UTF8_FULL);
    table.set_header(vec!["CN", "Description", "Distinguished Name"]);
    for g in groups {
        table.add_row(vec![
            g.cn.clone(),
            g.description.clone().unwrap_or_default(),
            g.dn.clone(),
        ]);
    }
    println!("{} group(s) total", groups.len());
    println!("{table}");
}

fn write_csv(groups: &[GroupRecord]) -> Result<()> {
    let mut wtr = csv::Writer::from_writer(std::io::stdout());
    wtr.write_record(["cn", "description", "dn"])
        .map_err(|e| AppError::Other(e.to_string()))?;
    for g in groups {
        wtr.write_record([
            g.cn.as_str(),
            g.description.as_deref().unwrap_or(""),
            g.dn.as_str(),
        ])
        .map_err(|e| AppError::Other(e.to_string()))?;
    }
    wtr.flush().map_err(AppError::Io)?;
    Ok(())
}

fn write_json(groups: &[GroupRecord]) -> Result<()> {
    let text = serde_json::to_string_pretty(groups).map_err(|e| AppError::Other(e.to_string()))?;
    println!("{text}");
    Ok(())
}

fn interactive_filter(groups: &[GroupRecord]) -> Result<()> {
    if groups.is_empty() {
        println!("This user is not a member of any group.");
        return Ok(());
    }

    let items: Vec<String> = groups.iter().map(|g| g.cn.clone()).collect();
    let prompt = format!(
        "{} group(s) — type to filter, Enter for detail, Esc to quit",
        groups.len()
    );

    loop {
        let selection = FuzzySelect::new()
            .with_prompt(&prompt)
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|e| AppError::Other(e.to_string()))?;

        let Some(index) = selection else {
            break;
        };
        let g = &groups[index];
        println!("\nCN:          {}", g.cn);
        println!("DN:          {}", g.dn);
        println!(
            "Description: {}\n",
            g.description.as_deref().unwrap_or("(none)")
        );
    }
    Ok(())
}
