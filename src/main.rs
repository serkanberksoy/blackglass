//! blackglass: an Obsidian-style note vault in the terminal, or in a
//! window of its own (`--gui`, the `gui` feature).
//!
//! Usage: blackglass [OPTIONS] [FOLDER | NOTE]

fn main() -> std::io::Result<()> {
    // `args_os`: a non-UTF-8 argument must not panic (it's converted lossily).
    let args = std::env::args_os()
        .skip(1)
        .map(|a| {
            a.into_string()
                .unwrap_or_else(|a| a.to_string_lossy().into_owned())
        })
        .collect();
    blackglass::launch::main(args)
}
