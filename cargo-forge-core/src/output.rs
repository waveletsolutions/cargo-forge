//! Colored terminal output helpers.

use colored::Colorize;

pub fn ok(msg: &str) {
    println!("{}   {}", "ok".green(), msg);
}

pub fn warn(msg: &str) {
    println!("{}  {}", "warn".yellow(), msg);
}

pub fn info(msg: &str) {
    println!("{}  {}", "....".cyan(), msg);
}

pub fn fail(msg: &str) {
    eprintln!("{}  {}", "FAIL".red(), msg);
}

pub fn header(msg: &str) {
    let line = format!("=== {} ===", msg);
    println!("\n{}\n", line.bold());
}

pub fn success(msg: &str) {
    println!("{}", msg.bold().green());
}