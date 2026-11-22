#![cfg_attr(windows, windows_subsystem = "windows")]
use anyhow::{Result, bail};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};
use yapfast_enhanced_manager as manager;

fn output_options(args: &mut Vec<OsString>) -> Result<(Option<PathBuf>, bool)> {
    // Everything after -- belongs to Yapfast, including untrusted protocol
    // arguments. Never reinterpret those as manager output-file options.
    let mut end = args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or(args.len());
    let mut at = 1;
    let mut report = None;
    let mut notify = false;
    while at < end {
        if args[at] == "--notify" {
            args.remove(at);
            end -= 1;
            notify = true;
        } else if args[at] == "--report" {
            if at + 1 >= end || report.is_some() {
                bail!("Invalid report option");
            }
            report = Some(PathBuf::from(args.remove(at + 1)));
            args.remove(at);
            end -= 2;
        } else {
            at += 1;
        }
    }
    Ok((report, notify))
}

fn run(mut args: Vec<OsString>) -> Result<String> {
    if args.is_empty() {
        bail!(
            "Use the Yapfast Enhanced installer. Commands: check, stop, install, restore, repair, managed-update"
        );
    }
    let command = args.remove(0);
    match (command.to_str(), args.as_slice()) {
        (Some("check"), [app, bundle]) => manager::check(Path::new(app), Path::new(bundle)),
        (Some("install"), [app, bundle]) => manager::install(Path::new(app), Path::new(bundle)),
        (Some("restore"), [app]) => manager::restore(Path::new(app)),
        (Some("cleanup"), [app]) => manager::maintenance::maintain(Path::new(app)),
        (Some("stop"), [app]) => manager::shutdown::stop(Path::new(app)),
        (Some("stop"), [app, bundle]) => {
            manager::stop_for_install(Path::new(app), Path::new(bundle))
        }
        (Some("repair"), [app]) => manager::updates::repair_online(Path::new(app)),
        (Some("managed-update"), [app, installer, separator, rest @ ..]) if separator == "--" => {
            manager::updates::managed_update(Path::new(app), Path::new(installer), rest)
        }
        _ => bail!("Invalid Enhanced manager arguments"),
    }
}

fn main() {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    let (report, interactive) = match output_options(&mut args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let managed = args.first().is_some_and(|value| value == "managed-update");
    let restart = if managed && args.len() >= 4 && args[3] == "--" {
        Some((PathBuf::from(&args[1]), args[4..].to_vec()))
    } else {
        None
    };
    let result = run(args);
    let (message, code) = match result {
        Ok(message) => (message, 0),
        Err(error) => (format!("{error:#}"), 1),
    };
    if let Some(report) = report
        && let Err(error) = std::fs::write(report, &message)
    {
        eprintln!("Cannot write report: {error}");
    }
    if code == 0 {
        println!("{message}");
    } else {
        eprintln!("{message}");
    }
    if code != 0
        && let Some((app, args)) = restart
        && manager::stopped_for_restart(&app)
    {
        let _ = manager::updates::restart(&app, &args);
    }
    if interactive || (managed && code != 0) {
        manager::updates::notify(&message);
    }
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forwarded_report_arguments_are_never_manager_options() {
        let mut args: Vec<OsString> = [
            "managed-update",
            "app",
            "installer",
            "--",
            "--open-link",
            "discord://example",
            "--report",
            "must-not-write",
            "--notify",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let original = args.clone();
        let (report, notify) = output_options(&mut args).unwrap();
        assert!(report.is_none());
        assert!(!notify);
        assert_eq!(args, original);
    }
    #[test]
    fn installer_report_and_notification_options_are_extracted() {
        let mut args: Vec<OsString> = [
            "install",
            "app folder",
            "bundle folder",
            "--report",
            "report file",
            "--notify",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let (report, notify) = output_options(&mut args).unwrap();
        assert_eq!(report, Some(PathBuf::from("report file")));
        assert!(notify);
        assert_eq!(args.len(), 3);
    }
}
