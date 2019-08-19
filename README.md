# server
`cargo build --release --manifest-path example/Cargo.toml`
`example/target/release/server 4 190000`
> `4` indicates the count of grpc thread
> `190000` indicates the len of map, it can control the size of response
> body

# client
We can use `grpcurl` to request the server
