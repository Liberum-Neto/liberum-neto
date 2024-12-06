#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR"/lib/asserts.sh

CORE_BIN=$1
CLI_BIN=$2

N1="test_n1"
N1_SEED=1
N1_ADDR="/ip6/::1/udp/52138/quic-v1"
N2="test_n2"
N2_SEED=2
FILE_NAME="$PWD/test-file.txt"
FILE_CONTENT="Hello, World!"
BLAKE3_HASH="5ckG8X2Ad8avzL57V5tvHbHpgxVtgAU5swwsxeMaNszx"

echo "Publish and download file test:"

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

# add n1 as bootstrap
$CLI_BIN -d config-node $N2 add-bootstrap-node "${N1_ID}" $N1_ADDR 2> /dev/null
$CLI_BIN -d start-node $N2 2> /dev/null

# wait for nodes to connect
sleep 0.1

# create and provide file
echo "${FILE_CONTENT}" > "$FILE_NAME"
$CLI_BIN -d publish-file $N1 "$FILE_NAME" &> /dev/null

init_asserts
# download file
RESULT=$($CLI_BIN -d download-file $N2 "${BLAKE3_HASH}" 2> /dev/null)
should_contain "$RESULT" "${FILE_CONTENT}"

# cleanup
$CLI_BIN -d stop-node $N1 2> /dev/null
$CLI_BIN -d stop-node $N2 2> /dev/null
killall liberum_core &> /dev/null
rm "$FILE_NAME"

exit $(check_asserts)
