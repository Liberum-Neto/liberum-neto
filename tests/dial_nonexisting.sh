#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR"/lib/asserts.sh

N1="test_n1"
N1_SEED=1
N1_ADDR="/ip6/::1/udp/53137/quic-v1"
N1_BAD_ADDR="/ip6/::2/udp/53137/quic-v1"
N1_BAD_PORT="/ip6/::1/udp/53199/quic-v1"
N2="test_n2"
N2_SEED=2

echo "Dial nonexisting node test:"

# run daemon
killall liberum_core &> /dev/null
cargo run -p liberum_core -- --daemon  &> /dev/null &
sleep 0.5; # the socket file is created asynchronously and may not be ready yet :))))

# create ndoes
cargo run -p liberum_cli -- -d new-node $N1 --id-seed $N1_SEED 2> /dev/null
cargo run -p liberum_cli -- -d new-node $N2 --id-seed $N2_SEED 2> /dev/null
cargo run -p liberum_cli -- -d config-node $N1 add-external-addr $N1_ADDR 2> /dev/null

# start n1 and get its peer id
cargo run -p liberum_cli -- -d start-node $N1 2> /dev/null
N1_ID=$(cargo run -p liberum_cli -- -d get-peer-id $N1 2> /dev/null)

cargo run -p liberum_cli -- -d start-node $N2 2> /dev/null

init_asserts

# dial nonexisting addresses
RESULT1=$(cargo run -p liberum_cli dial $N2 "${N1_ID}" $N1_BAD_ADDR 2> /dev/null)
should_be_equal "$RESULT1" "Error dialing peer"
RESULT2=$(cargo run -p liberum_cli dial $N2 "${N1_ID}" $N1_BAD_PORT 2> /dev/null)
should_be_equal "$RESULT2" "Error dialing peer"

# dial real address
RESULT3=$(cargo run -p liberum_cli dial $N2 "${N1_ID}" $N1_ADDR 2> /dev/null)
should_be_equal "$RESULT3" "Dialing successful"

# nodes should not die
ALIVE=$(cargo run -p liberum_cli -- -d list-nodes 2> /dev/null | grep -c "true")
should_be_equal "$ALIVE" "2"

# cleanup
cargo run -p liberum_cli -- -d stop-node $N1 2> /dev/null
cargo run -p liberum_cli -- -d stop-node $N2 2> /dev/null
killall liberum_core &> /dev/null

exit $(check_asserts)
