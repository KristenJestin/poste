//! Binaire `poste-display` (`poste display …`) : voir la bibliothèque.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(poste_display::run(&argv));
}
