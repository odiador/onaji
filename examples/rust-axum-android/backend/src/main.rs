fn main() {
    println!("Axum Backend Service initialized on http://127.0.0.1:3000");
}

pub fn health_check() -> &'static str {
    "OK"
}

pub fn list_users() -> &'static str {
    "[]"
}
