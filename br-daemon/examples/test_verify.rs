use bcrypt::verify;

fn main() {
    let password = "admin";
    let hash = "$2b$12$6jrUOJoVTdda9d2zrg7SSuWmEchZ9vCplFt3jEKHMZLddSBXGhZY.";

    println!("Testing password verification:");
    println!("Password: {}", password);
    println!("Hash: {}", hash);

    match verify(password, hash) {
        Ok(true) => println!("Result: MATCH!"),
        Ok(false) => println!("Result: NO MATCH"),
        Err(e) => println!("Error: {:?}", e),
    }
}
