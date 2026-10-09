//! `blackglass-window`: blackglass in its window, without a console
//! (Windows: double-click it, or pin it to the Start menu). The same as
//! `blackglass --gui`; built for the Windows release (the
//! `windows-launcher` feature).

#![windows_subsystem = "windows"]

fn main() -> std::io::Result<()> {
    let args = std::iter::once("--gui".to_string())
        .chain(std::env::args_os().skip(1).map(|a| {
            a.into_string()
                .unwrap_or_else(|a| a.to_string_lossy().into_owned())
        }))
        .collect();
    blackglass::launch::main(args)
}
