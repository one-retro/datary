//! Prints a summary of a datafile.
//!
//! ```sh
//! cargo run --example info -- tests/fixtures/no-intro/virtual-boy.dat
//! ```

use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: info <datafile.dat>");
        std::process::exit(2);
    };

    let dat = datary::read_file(&path)?;

    if let Some(header) = &dat.header {
        println!("{} ({})", header.name, header.version);
        println!("  description : {}", header.description);
        println!("  author      : {}", header.author);
        if let Some(id) = header.id {
            println!("  no-intro id : {id}");
        }
        if let Some(url) = &header.url {
            println!("  url         : {url}");
        }
    }

    let total_size: u64 = dat.games.iter().map(datary::Game::total_size).sum();
    println!("\n{} games, {} roms", dat.games.len(), dat.rom_count());
    println!(
        "total size: {:.2} MiB",
        total_size as f64 / (1024.0 * 1024.0)
    );

    let clones = dat.games.iter().filter(|g| g.is_clone()).count();
    let mia = dat.games.iter().filter(|g| g.is_mia()).count();
    println!("{clones} clones, {mia} missing in action");

    let mut categories: BTreeMap<&str, usize> = BTreeMap::new();
    for game in &dat.games {
        for category in &game.categories {
            *categories.entry(category.as_str()).or_default() += 1;
        }
    }
    if !categories.is_empty() {
        println!("\ncategories:");
        for (name, count) in categories {
            println!("  {count:>5}  {name}");
        }
    }

    let mut statuses: BTreeMap<String, usize> = BTreeMap::new();
    for (_, rom) in dat.roms() {
        *statuses.entry(format!("{:?}", rom.status())).or_default() += 1;
    }
    println!("\nrom status:");
    for (status, count) in statuses {
        println!("  {count:>5}  {status}");
    }

    Ok(())
}
