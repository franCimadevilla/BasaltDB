use std::io::{stdin,stdout, Write};

fn main() {
    let stdin = stdin();

    loop {
        print!("> ");
        stdout().flush().unwrap();

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