#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR"/lib/asserts.sh

CORE_BIN=$1
CLI_BIN=$2

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
$CORE_BIN --daemon  &> /dev/null &
sleep 0.1; # the socket file is created asynchronously and may not be ready yet :))))

# create ndoes
$CLI_BIN -d new-node $N1 --id-seed $N1_SEED 2> /dev/null
$CLI_BIN -d new-node $N2 --id-seed $N2_SEED 2> /dev/null
$CLI_BIN -d config-node $N1 add-external-addr $N1_ADDR 2> /dev/null

# start n1 and get its peer id
$CLI_BIN -d start-node $N1 2> /dev/null
N1_ID=$($CLI_BIN -d get-peer-id $N1 2> /dev/null)

$CLI_BIN -d start-node $N2 2> /dev/null

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
ALIVE=$($CLI_BIN -d list-nodes 2> /dev/null | grep -c "true")
should_be_equal "$ALIVE" "2"

# cleanup
$CLI_BIN -d stop-node $N1 2> /dev/null
$CLI_BIN -d stop-node $N2 2> /dev/null
killall liberum_core &> /dev/null

exit $(check_asserts)
