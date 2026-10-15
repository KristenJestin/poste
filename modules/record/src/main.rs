//! `poste-record` binary (`poste record …`): see the library for the commands.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(poste_record::run(&argv));
}
