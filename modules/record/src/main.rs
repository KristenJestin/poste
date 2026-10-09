//! Binaire `poste-record` (`poste record …`) : voir la bibliothèque pour les commandes.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(poste_record::run(&argv));
}
