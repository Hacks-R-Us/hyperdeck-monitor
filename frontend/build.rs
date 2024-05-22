fn main() {
    // Sorry, I know it's horrible but I don't have the spoons to make it right.
    const API_DEFINITION_FILE: &str = "../src/api/message.rs";
    println!("cargo:rerun-if-changed={API_DEFINITION_FILE}");
    std::fs::copy(API_DEFINITION_FILE, "./src/websocket.rs")
        .expect("Failed to copy API definition");
}
