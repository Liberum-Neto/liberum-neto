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
N2_ADDR="/ip6::1/udp/52139/quic-v1"

FILE_NAME="$PWD/test-file.txt"
FILE_CONTENT="Hello, World!"

echo "Publish and download file test:"

# run daemon
# killall liberum_core &> /dev/null
# $CORE_BIN --daemon  &> /dev/null &
# sleep 0.1; # the socket file is created asynchronously and may not be ready yet :))))

# create ndoes
$CLI_BIN -d new-node $N1 --id-seed $N1_SEED
$CLI_BIN -d new-node $N2 --id-seed $N2_SEED
$CLI_BIN -d config-node $N1 add-external-addr $N1_ADDR
$CLI_BIN -d config-node $N2 add-external-addr $N2_ADDR

# start n1 and get its peer id
$CLI_BIN -d start-node $N1
N1_ID=$($CLI_BIN -d get-peer-id $N1)

# add n1 as bootstrap
$CLI_BIN -d config-node $N2 add-bootstrap-node "${N1_ID}" $N1_ADDR
$CLI_BIN -d start-node $N2

# wait for nodes to connect
sleep 0.1

# create and publish file
echo "${FILE_CONTENT}" > "$FILE_NAME"
FILE_ID=$($CLI_BIN publish-file $N1 "$FILE_NAME")

init_asserts
# download file
RESULT=$($CLI_BIN -d download-file $N1 "${FILE_ID}")
should_contain "$RESULT" "${FILE_CONTENT}"

# $DELETE_FAIL=$($CLI_BIN -d delete-object $N2 "${FILE_ID}")
# should_contain "$DELETE_FAIL" "Failed"
DELETE_SUCCESS=$($CLI_BIN -d delete-object $N1 "${FILE_ID}")
should_contain "$DELETE_SUCCESS" "Success"

RESULT2=$($CLI_BIN -d download-file $N2 "${FILE_ID}")
should_contain "$RESULT2" "Failed"

# cleanup
$CLI_BIN -d stop-node $N1
$CLI_BIN -d stop-node $N2
killall liberum_core &> /dev/null
rm "$FILE_NAME"

exit $(check_asserts)
