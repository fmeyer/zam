use alias::Alias;
use db::Database;
use clap::{App, Arg, SubCommand, AppSettings};
use std::process;

mod alias;
mod db;

fn main() {
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
                .arg(Arg::with_name("shell").required(true))
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
        .subcommand(SubCommand::with_name("list").about("List all aliases"))
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

    // TODO(fm) : retrieve database from dotfiles or some
    let db = Database::new("sample/config.zam").unwrap_or_else(|err| {
        eprintln!("Error initializing database: {}", err);
        process::exit(1);
    });

    match matches.subcommand() {
        Some(("add", add_matches)) => {
            let alias = add_matches.value_of("alias").unwrap();
            let command = add_matches.value_of("command").unwrap();
            let shell = add_matches.value_of("shell").unwrap();
            let description = add_matches.value_of("description").unwrap();

            let new_alias = Alias::new(
                alias.to_string(),
                command.to_string(),
                shell.to_string(),
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
                "".to_string(),
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
        Some(("list", _)) => {
            let aliases = db.list_aliases().unwrap_or_else(|err| {
                eprintln!("Error listing aliases: {}", err);
                process::exit(1);
            });

            for alias in aliases {
                println!(
                    "{}: {} ({}) [{}]",
                    alias.alias, alias.command, alias.shell, alias.description
                );
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
