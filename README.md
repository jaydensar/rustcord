# rustcord

Rustcord is a barebones chat application written in 100% Rust.

## Running the program

0. Install Rust; you can use [`rustup`](https://rustup.rs/) for this. Follow the instructions on the `rustup` page and ensure you have `cargo` in your PATH afterwards.

1. Generate the database code with `cargo prisma generate`, then generate the `.db` file with `cargo prisma migrate dev`.

2. Copy the `.env.example` file, edit the `JWT_SECRET` variable with a random, secure value such as `@8ojPLy7t$8!H7`, then name the file `.env`.

3. Run the server with `cargo run --bin server`

4. Run the client with `cargo run --bin client`
