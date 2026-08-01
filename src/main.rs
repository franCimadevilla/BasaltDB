use std::io::{self, Write};

fn main() {
    let stdin = io::stdin();

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();

        let input = input.trim();

        if input == "quit" {
            println!("Exiting...");
            break;
        }

        println!("Unrecogniced input: {}", input);
    }
}