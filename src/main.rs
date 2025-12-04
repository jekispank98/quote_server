use std::net::TcpListener;

pub mod server;

pub mod model;

const ADDRESS: &str = "127.0.0.1:8080";
fn main() {
    let listener = TcpListener::bind(ADDRESS);
    println!("Server listening on port 8080");


}
