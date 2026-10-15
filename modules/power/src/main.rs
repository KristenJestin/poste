//! Binary `poste-power` (`poste power …`): see the library.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(poste_power::run(&argv));
}
