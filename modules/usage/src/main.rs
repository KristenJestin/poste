//! Binaire `poste-usage` (`poste usage …`) : voir la bibliothèque.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(poste_usage::run(&argv));
}
