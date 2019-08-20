# server
```
cd protoc-grpcio
cargo build --release
target/release/server 128
```
> `128` indicates the count of grpc thread

# client
Use `grpcurl` to request the server

```
cd protoc-grpcio
while true; do grpcurl -max-msg-sz 419430400 -plaintext -import-path src/protos/example/ -proto diner.proto -d '{}' 127.0.0.1:5900 example.Diner.Eat > e.log; done
```
