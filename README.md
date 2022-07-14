# rustcord

Rustcord is a barebones chat application written in 100% Rust.

## Running the program

0. Install Rust; you can use [`rustup`](https://rustup.rs/) for this. Follow the instructions on the `rustup` page and ensure you have `cargo` in your PATH afterwards.

1. Generate the database code with `cargo prisma generate`, then generate the `.db` file with `cargo prisma migrate dev`.

2. Copy the `.env.example` file, edit the `JWT_SECRET` variable with a random, secure value such as `@8ojPLy7t$8!H7`, then name the file `.env`.

3. Run the server with `cargo run --bin server`

4. Run the client with `cargo run --bin client`

## Issues
My only concern was getting this in a "working" state as fast as possible, and this was a school project, so there are a ton of issues. Here are a few of the biggest ones:
* Blocking HTTP: by far the most glaring issue — http requests are issued with reqwest blocking, and since they’re issued in the same thread as the UI, it momentarily freezes the entire UI. This also prevents Rustcord from running in the browser as a WASM application.
* Nonexistent error handling: the entire client application panics and crashes if you trigger a non-2xx HTTP response, such as trying to log in to an account that doesn’t exist.
* Partially implemented authentication: Rustcord uses JWTs for authentication. I was planning on implementing a refresh token mechanism, but never got to it, so JWTs just last for 7 days with no way to revoke them.
* Code quality: The entire client is a 700 line file, some code sections are duplicated in both the client and server. Code quality can easily be improved to be more idiomatic, performant, and contain less duplicate code.

