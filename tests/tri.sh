#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR"/lib/asserts.sh

CORE_BIN=$1
CLI_BIN=$2

N1="test_n1"
N1_SEED=1
N1_ADDR="/ip6/::1/udp/52148/quic-v1"
N2="test_n2"
N2_SEED=2
N2_ADDR="/ip6/::1/udp/52149/quic-v1"
N3="test_n3"
N3_SEED=3

FILE_NAME="$PWD/test-file.txt"
FILE_CONTENT="Hello, World!"

echo "Publish and download file test:"

# run daemon
killall liberum_core &> /dev/null
$CORE_BIN --daemon  &> /dev/null &
sleep 0.1; # the socket file is created asynchronously and may not be ready yet :))))

# create ndoes
$CLI_BIN -d new-node $N1 --id-seed $N1_SEED 2> /dev/null
$CLI_BIN -d new-node $N2 --id-seed $N2_SEED 2> /dev/null
$CLI_BIN -d new-node $N3 --id-seed $N3_SEED 2> /dev/null
$CLI_BIN -d config-node $N1 add-external-addr $N1_ADDR 2> /dev/null

# start n1 and get its peer id
$CLI_BIN -d start-node $N1 2> /dev/null
N1_ID=$($CLI_BIN -d get-peer-id $N1 2> /dev/null)

# add n1 as bootstrap
$CLI_BIN -d config-node $N2 add-bootstrap-node "${N1_ID}" $N1_ADDR 2> /dev/null
$CLI_BIN -d config-node $N2 add-external-addr $N2_ADDR 2> /dev/null
$CLI_BIN -d start-node $N2 2> /dev/null
N2_ID=$($CLI_BIN -d get-peer-id $N2 2> /dev/null)

$CLI_BIN -d config-node $N3 add-bootstrap-node "${N1_ID}" $N1_ADDR 2> /dev/null
$CLI_BIN -d start-node $N3 2> /dev/null

# wait for nodes to connect
sleep 0.1

$CLI_BIN -d dial $N3 $N2_ID $N2_ADDR
