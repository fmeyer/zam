use alias::Alias;
use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use db::Database;
use prettytable::{format, Table};
use std::env;
use std::fs;
use std::path::Path;
use std::process;

mod alias;
mod db;

const ZAM_DATABASE_FILE: &str = "ZAM_DATABASE_FILE";
const DEFAULT_ZAM_FILE: &str = "zam.db";

fn main() {
    let db = load_config();
    let matches = build_command_options();
    match matches.subcommand() {
        Some(("add", add_matches)) => {
            let alias = add_matches.value_of("alias").unwrap();
            let command = add_matches.value_of("command").unwrap();
            let description = add_matches.value_of("description").unwrap();

            let new_alias = Alias::new(
                alias.to_string(),
                command.to_string(),
                description.to_string(),
            );
            db.add_alias(&new_alias).unwrap_or_else(|err| {
                eprintln!("Error adding alias: {}", err);
                process::exit(1);
            });
            println!("Alias added successfully");
        }
        Some(("update", update_matches)) => {
            let alias = update_matches.value_of("alias").unwrap();
            let command = update_matches.value_of("command").unwrap();
            let description = update_matches.value_of("description").unwrap();

            let mut alias_to_update = Alias::new(
                alias.to_string(),
                command.to_string(),
                description.to_string(),
            );
            alias_to_update.update(command.to_string());

            db.update_alias(&alias_to_update).unwrap_or_else(|err| {
                eprintln!("Error updating alias: {}", err);
                process::exit(1);
            });
            println!("Alias updated successfully");
        }
        Some(("remove", remove_matches)) => {
            let alias = remove_matches.value_of("alias").unwrap();

            db.remove_alias(alias).unwrap_or_else(|_err| {
                eprintln!("Alias removed successfully");
            });
        }
        Some(("display", _)) => {
            let mut table =
                Table::from_csv_string(db.export_aliases_to_csv_buffer().unwrap().as_str())
                    .unwrap();
            table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
            table.printstd();
        }
        Some(("aliases", _)) => {
            let aliases = db.list_aliases().unwrap_or_else(|err| {
                eprintln!("Error listing aliases: {}", err);
                process::exit(1);
            });

            for alias in aliases {
                println!("alias {}='{}'", alias.alias, alias.command);
            }
        }
        Some(("export", export_matches)) => {
            let file = export_matches.value_of("file").unwrap();

            db.export_aliases_to_csv(file).unwrap_or_else(|err| {
                eprintln!("Error exporting aliases: {}", err);
                process::exit(1);
            });
            println!("Aliases exported successfully");
        }
        Some(("import", import_matches)) => {
            let file = import_matches.value_of("file").unwrap();

            db.import_aliases_from_csv(file).unwrap_or_else(|err| {
                eprintln!("Error importing aliases: {}", err);
                process::exit(1);
            });
            println!("Aliases imported successfully");
        }
        _ => {
            eprintln!("Invalid command");
            process::exit(1);
        }
    }
}

fn build_command_options() -> ArgMatches {
    let matches = App::new("Alias Manager")
        .version("0.0.1")
        .author("Fernando Meyer <fm@pobox.com>")
        .about("Zsh Alias Manager")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("add")
                .about("Add a new alias")
                .arg(Arg::with_name("alias").required(true))
                .arg(Arg::with_name("command").required(true))
                .arg(Arg::with_name("description").required(true)),
        )
        .subcommand(
            SubCommand::with_name("update")
                .about("Update an existing alias")
                .arg(Arg::with_name("alias").required(true))
                .arg(Arg::with_name("command").required(true))
                .arg(Arg::with_name("description").required(true)),
        )
        .subcommand(
            SubCommand::with_name("remove")
                .about("Remove an alias")
                .arg(Arg::with_name("alias").required(true)),
        )
        .subcommand(
            SubCommand::with_name("aliases").about("List all aliases in shell `eval` ready format"),
        )
        .subcommand(
            SubCommand::with_name("display").about("List all aliases in descriptive format"),
        )
        .subcommand(
            SubCommand::with_name("export")
                .about("Export aliases to a CSV file")
                .arg(Arg::with_name("file").required(true)),
        )
        .subcommand(
            //TODO(fm): import won't properly handle duplicates
            SubCommand::with_name("import")
                .about("Import aliases from a CSV file")
                .arg(Arg::with_name("file").required(true)),
        )
        .get_matches();
    matches
}

fn load_config() -> Database {
    let config_dir = ensure_config_directory();
    let db_path = Path::new(&config_dir).join(DEFAULT_ZAM_FILE);

    let zam_database_file = match db_path.exists() {
        true => db_path.to_string_lossy().to_string(),
        false => match env::var(ZAM_DATABASE_FILE) {
            Ok(env_value) => env_value,
            Err(_) => {
                println!("There's no existing database or environment variable {} is set. Creating a new database.", ZAM_DATABASE_FILE);
                db_path.to_string_lossy().to_string()
            }
        },
    };

    let db = Database::new(zam_database_file).unwrap_or_else(|err| {
        eprintln!("Error initializing database: {}", err);
        process::exit(1);
    });
    db
}

fn ensure_config_directory() -> String {
    let config_dir = format!(
        "{}/.config/zam",
        env::var("HOME").expect("HOME env not available")
    );
    fs::create_dir_all(&config_dir)
        .expect("Can't create directory, are you sure you have permission for running this code?");

    config_dir
}
