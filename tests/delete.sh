#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR"/lib/asserts.sh

CORE_BIN=$1
CLI_BIN=$2

N1="test_n1"
N1_SEED=1
N1_ADDR="/ip6/::1/udp/53138/quic-v1"
N2="test_n2"
N2_SEED=2
N2_ADDR="/ip6::1/udp/53139/quic-v1"
N3="test_n3"
N3_SEED=3
N3_ADDR="/ip6::1/udp/531140/quic-v1"

FILE_NAME="$PWD/test-file.txt"
FILE_CONTENT="Hello, World!"

echo "Publish and download file test:"

# run daemon
killall liberum_core &> /dev/null
$CORE_BIN --daemon  &> /dev/null &
sleep 0.1; # the socket file is created asynchronously and may not be ready yet :))))

# create ndoes
$CLI_BIN -d new-node $N1 --id-seed $N1_SEED
$CLI_BIN -d new-node $N2 --id-seed $N2_SEED
$CLI_BIN -d new-node $N3 --id-seed $N3_SEED
$CLI_BIN -d config-node $N1 add-external-addr $N1_ADDR
$CLI_BIN -d config-node $N2 add-external-addr $N2_ADDR
$CLI_BIN -d config-node $N3 add-external-addr $N3_ADDR

# start n1 and get its peer id
$CLI_BIN -d start-node $N1
N1_ID=$($CLI_BIN -d get-peer-id $N1)

# add n1 as bootstrap
$CLI_BIN -d config-node $N2 add-bootstrap-node "${N1_ID}" $N1_ADDR
$CLI_BIN -d config-node $N3 add-bootstrap-node "${N1_ID}" $N1_ADDR
$CLI_BIN -d start-node $N2
$CLI_BIN -d start-node $N3

# wait for nodes to connect
sleep 0.1

# # create and publish file
echo "${FILE_CONTENT}" > "$FILE_NAME"
FILE_ID=$($CLI_BIN publish-file $N1 "$FILE_NAME")

# init_asserts
# download file
DOWNLOAD_1_SUCCESS=$($CLI_BIN -d download-file $N1 "${FILE_ID}")
should_contain "$DOWNLOAD_1_SUCCESS" "${FILE_CONTENT}"
DOWNLOAD_2_SUCCESS=$($CLI_BIN -d download-file $N2 "${FILE_ID}")
should_contain "$DOWNLOAD_2_SUCCESS" "${FILE_CONTENT}"
DOWNLOAD_3_SUCCESS=$($CLI_BIN -d download-file $N3 "${FILE_ID}")
should_contain "$DOWNLOAD_3_SUCCESS" "${FILE_CONTENT}"

# # delete file
DELETE_3_FAIL=$($CLI_BIN -d delete-object $N3 "${FILE_ID}")
should_contain "$DELETE_3_FAIL" "Deleted myself: true"
should_contain "$DELETE_3_FAIL" "Successful deletes: 0"
should_contain "$DELETE_3_FAIL" "Failed deletes: 1"
DELETE_2_FAIL=$($CLI_BIN -d delete-object $N2 "${FILE_ID}")
should_contain "$DELETE_2_FAIL" "Deleted myself: true"
should_contain "$DELETE_2_FAIL" "Successful deletes: 0"
should_contain "$DELETE_2_FAIL" "Failed deletes: 1"
DELETE_1_FAIL=$($CLI_BIN -d delete-object $N1 "${FILE_ID}")
should_contain "$DELETE_1_FAIL" "Deleted myself: false"
should_contain "$DELETE_1_FAIL" "Successful deletes: 0"
should_contain "$DELETE_1_FAIL" "Failed deletes: 2"

# # try to download file
DOWNLOAD_1_FAIL=$($CLI_BIN -d download-file $N1 "${FILE_ID}")
should_contain "$DOWNLOAD_1_FAIL" "Fail"
DOWNLOAD_2_FAIL=$($CLI_BIN -d download-file $N2 "${FILE_ID}")
should_contain "$DOWNLOAD_2_FAIL" "Fail"
DOWNLOAD_3_FAIL=$($CLI_BIN -d download-file $N3 "${FILE_ID}")
should_contain "$DOWNLOAD_3_FAIL" "Fail"

# publish
FILE_ID=$($CLI_BIN publish-file $N1 "$FILE_NAME")

# # try to download
DOWNLOAD_1_SUCCESS=$($CLI_BIN -d download-file $N1 "${FILE_ID}")
should_contain "$DOWNLOAD_1_SUCCESS" "${FILE_CONTENT}"
DOWNLOAD_2_SUCCESS=$($CLI_BIN -d download-file $N2 "${FILE_ID}")
should_contain "$DOWNLOAD_2_SUCCESS" "${FILE_CONTENT}"
DOWNLOAD_3_SUCCESS=$($CLI_BIN -d download-file $N3 "${FILE_ID}")
should_contain "$DOWNLOAD_3_SUCCESS" "${FILE_CONTENT}"

# delete
DELETE_1_SUCCESS=$($CLI_BIN -d delete-object $N1 "${FILE_ID}")
should_contain "$DELETE_1_SUCCESS" "Deleted myself: false"
should_contain "$DELETE_1_SUCCESS" "Successful deletes: 2"
should_contain "$DELETE_1_SUCCESS" "Failed deletes: 0"

# try to download
DOWNLOAD_1_FAIL=$($CLI_BIN -d download-file $N1 "${FILE_ID}")
should_contain "$DOWNLOAD_1_FAIL" "Fail"
DOWNLOAD_2_FAIL=$($CLI_BIN -d download-file $N2 "${FILE_ID}")
should_contain "$DOWNLOAD_2_FAIL" "Fail"
DOWNLOAD_3_FAIL=$($CLI_BIN -d download-file $N3 "${FILE_ID}")
should_contain "$DOWNLOAD_3_FAIL" "Fail"

# cleanup
$CLI_BIN -d stop-node $N1
$CLI_BIN -d stop-node $N2
killall liberum_core &> /dev/null
rm "$FILE_NAME"

exit $(check_asserts)
