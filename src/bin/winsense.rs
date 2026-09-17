use std::env;

fn main() {
    let mut action = "overview";
    let args: Vec<String> = env::args().collect();
    for i in 1..args.len() {
        if args[i] == "--action" && i + 1 < args.len() {
            action = &args[i + 1];
        } else if args[i].starts_with("--action=") {
            action = &args[i]["--action=".len()..];
        }
    }
    let output = presence::winsense::query(action);
    println!("{output}");
}
