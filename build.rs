fn main() {
    // rust-i18n embeds locale files at compile time via the `i18n!` macro in
    // src/main.rs; without this, editing a YAML file doesn't trigger re-expansion.
    println!("cargo:rerun-if-changed=locales");
}
