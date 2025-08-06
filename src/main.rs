fn main() {
    let mode = std::env::var("MODE").unwrap_or_else(|_| "api".to_string());
    let lb_port = std::env::var("PORT")
        .unwrap_or_else(|_| "9999".to_string())
        .parse::<u16>()
        .expect("Invalid port number");

    let socket_dir =
        std::env::var("SOCKET_DIR").expect("SOCKET_DIR environment variable is required");

    match mode.as_str() {
        "worker" => {
            worker::start(socket_dir);
        }
        _ => {
            println!("Starting in Load Balance mode on port: {lb_port}");
            load_balance::start(lb_port, socket_dir);
        }
    }
}
