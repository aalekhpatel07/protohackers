# Deployment

SSH on EC2 and run the server.

(Notes for self)
---

There's an EC2 instance accessible (for me) with `ssh ec2-protohackers`.
Inside the `~/protohackers` checkout, pull new changes on `main`, and run a release mode build:

```sh
RUST_LOG="protohackers=trace" cargo run --release
```

This runs servers for all the solutions at ports 12000, 12001, ... (in the order of the problem).

The IP of the server is of the EC2 instance, so login to AWS for that.

