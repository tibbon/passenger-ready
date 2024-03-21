# passenger-ready

This offers a health check to say if passenger is ready to receive more traffic, or if it's queue is full.

It can use environment variables to configure the port it runs on, and the max size of the pool. It returns false if the pool already reports it's capacity to be 80% full.

## How to use

`cargo test`

`cargo build`

`cargo run`

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.