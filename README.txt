How to run:

1) Install rust:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
2) Install libssl-dev
sudo apt instal libssl-dev
3) Select rust nightly
rustup default nightly
5) Build & run
./run.sh
6) You might need apache2-utils for testing
sudo apt install apache2-utils

Interface:
Since time was limited, I implemented simplistic REST interface.
localhost:8000/exclude/<i> will exclude instance #i (0 based)
localhost:8000/include/<i> will re-include this instance.
Heartbeat check will re-include any healthy instance automatically, as in problem statement.

localhost:8000/get is the get() method described in problem statement.
localhost:8000/status offers a serialized version of load_balancer status structure.

How to test:
The script starts a load_balancer along with 10 providers.

one can call:
ab -n 1000 -c 50 http://localhost:8000/get

to see results like this:
Concurrency Level:      50
Time taken for tests:   2.924 seconds
Complete requests:      1000
Failed requests:        0

Increasing concurrency above capacity of balancer, gives different results.
ps -aux | grep provider
kill -9 <some_provider_pid>

Concurrency Level:      50
Time taken for tests:   0.171 seconds
Complete requests:      1000
Failed requests:        834

There is much more failed requests are these happen significantly faster than successful ones.

