pub fn title(value: &str) {
    println!("{value}");
}

pub fn field(label: &str, value: impl std::fmt::Display) {
    println!("{label}: {value}");
}

pub fn blank() {
    println!();
}

pub fn note(value: &str) {
    println!("Note:");
    println!("{value}");
}
