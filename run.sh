#!/usr/bin/env bash
set -e -x

cargo build --release

instance_list=""

function ctrl_c() {
    echo "Shutting down."
    kill -TERM $(jobs -p)
    exit 0
}
trap ctrl_c INT

for i in {1..10}
do
  port=$((8000 + i))
  ./target/release/provider --port $port > /dev/null &
  instance_list+="localhost:$port,"
done

#providers=$(jobs -p)

./target/release/load_balancer --instances $instance_list --port 8000 &

#kill -TERM $providers
wait $(jobs -p)



