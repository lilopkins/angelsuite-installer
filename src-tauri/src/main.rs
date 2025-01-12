// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                chrono::Local::now(),
                record.level(),
                record.target(),
                message
            ))
        })
        //.level(log::LevelFilter::Info)
        // Additional logging to diagnose #15
        .level(log::LevelFilter::Trace)
        .level_for("angelsuite_installer", log::LevelFilter::Debug)
        .level_for("angelsuite_installer_lib", log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .chain(fern::log_file("angelsuite-installer.log").expect("Couldn't open log file."))
        .apply()
        .expect("Couldn't start logger!");

    angelsuite_installer_lib::run()
}
