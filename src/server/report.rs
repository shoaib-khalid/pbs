use std::path::Path;
use std::process::Command;

use crate::config::datastore;

fn files() -> Vec<&'static str> {
    vec![
        "/etc/hostname",
        "/etc/hosts",
        "/etc/network/interfaces",
        "/etc/proxmox-backup/datastore.cfg",
        "/etc/proxmox-backup/user.cfg",
        "/etc/proxmox-backup/acl.cfg",
        "/etc/proxmox-backup/remote.cfg",
        "/etc/proxmox-backup/sync.cfg",
        "/etc/proxmox-backup/verification.cfg",
    ]
}

fn commands() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
    //  ("<command>", vec![<arg [, arg]>])
        ("proxmox-backup-manager", vec!["versions", "--verbose"]),
        ("df", vec!["-h"]),
        ("lsblk", vec!["--ascii"]),
        ("zpool", vec!["status"]),
        ("zfs", vec!["list"]),
        ("proxmox-backup-manager", vec!["subscription", "get"]),
    ]
}

    // (<description>, <function to call>)
fn function_calls() -> Vec<(&'static str, fn() -> String)> {
    vec![
        ("Datastores", || {
            let config = match datastore::config() {
                Ok((config, _digest)) => config,
                _ => return String::from("could not read datastore config"),
            };

            let mut list = Vec::new();
            for store in config.sections.keys() {
                list.push(store.as_str());
            }
            list.join(", ")
        })
    ]
}

pub fn generate_report() -> String {
    use proxmox::tools::fs::file_read_optional_string;

    let file_contents = files()
        .iter()
        .map(|file_name| {
            let content = match file_read_optional_string(Path::new(file_name)) {
                Ok(Some(content)) => content,
                Ok(None) => String::from("# file does not exist"),
                Err(err) => err.to_string(),
            };
            format!("$ cat '{}'\n{}", file_name, content)
        })
        .collect::<Vec<String>>()
        .join("\n\n");

    let command_outputs = commands()
        .iter()
        .map(|(command, args)| {
            let output = Command::new(command)
                .env("PROXMOX_OUTPUT_NO_BORDER", "1")
                .args(args)
                .output();
            let output = match output {
                Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
                Err(err) => err.to_string(),
            };
            format!("$ `{} {}`\n{}", command, args.join(" "), output)
        })
        .collect::<Vec<String>>()
        .join("\n\n");

    let function_outputs = function_calls()
        .iter()
        .map(|(desc, function)| format!("$ {}\n{}", desc, function()))
        .collect::<Vec<String>>()
        .join("\n\n");

    format!(
        "= FILES =\n\n{}\n= COMMANDS =\n\n{}\n= FUNCTIONS =\n\n{}\n",
        file_contents, command_outputs, function_outputs
    )
}
