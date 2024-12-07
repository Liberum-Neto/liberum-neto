#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR"/lib/asserts.sh

CORE_BIN=$1
CLI_BIN=$2

N1="test_n1"
N1_SEED=1
N1_ADDR="/ip6/::1/udp/52137/quic-v1"
FILE1_NAME="$PWD/test-file1.txt"
FILE1_CONTENT="11111 Hello, World! 11111"
FILE1_HASH="EKMp67LEX2hBuejfSEgb48pNsFN6KZcqsnCAKf8JpdUH"

N2="test_n2"
N2_SEED=2
N2_ADDR="/ip6/::1/udp/52138/quic-v1"
FILE2_NAME="$PWD/test-file2.txt"
FILE2_CONTENT="22222 Hello, World! 22222"
FILE2_HASH="FyrNdKeJZf1QPvym8kPx3D69q7WWMcwKWkMybkp1Coyx"

N3="test_n3"
N3_SEED=3
N3_ADDR="/ip6/::1/udp/52139/quic-v1"
FILE3_NAME="$PWD/test-file3.txt"
FILE3_CONTENT="33333 Hello, World! 33333"
FILE3_HASH="8Yucj7Box5FDq4mmwp7RZHH8Aymd5Puwpb7QQCpaAYuZ"

N4="test_n4"
N4_SEED=4
N4_ADDR="/ip6/::1/udp/52140/quic-v1"
FILE4_NAME="$PWD/test-file4.txt"
FILE4_CONTENT="44444 Hello, World! 44444"
FILE4_HASH="4KQZTD1ejdyZr1sJ5by6WQyfEPtaU1Tib57mDeCEaiW8"

cleanup () {
    $CLI_BIN -d stop-node $N1 2> /dev/null
    $CLI_BIN -d stop-node $N2 2> /dev/null
    $CLI_BIN -d stop-node $N3 2> /dev/null
    $CLI_BIN -d stop-node $N4 2> /dev/null
    killall liberum_core &> /dev/null
    rm "$FILE1_NAME"
    rm "$FILE2_NAME"
    rm "$FILE3_NAME"
    rm "$FILE4_NAME"
}

echo "Provide and download file ULTRA test:"

# run daemon
killall liberum_core &> /dev/null
$CORE_BIN --daemon  &> /dev/null &
sleep 0.1; # the socket file is created asynchronously and may not be ready yet :))))

# create ndoes
$CLI_BIN -d new-node $N1 --id-seed $N1_SEED 2> /dev/null
$CLI_BIN -d new-node $N2 --id-seed $N2_SEED 2> /dev/null
$CLI_BIN -d new-node $N3 --id-seed $N3_SEED 2> /dev/null
$CLI_BIN -d new-node $N4 --id-seed $N4_SEED 2> /dev/null

#config addresses
$CLI_BIN -d config-node $N1 add-external-addr $N1_ADDR 2> /dev/null
$CLI_BIN -d config-node $N2 add-external-addr $N2_ADDR 2> /dev/null
$CLI_BIN -d config-node $N3 add-external-addr $N3_ADDR 2> /dev/null
$CLI_BIN -d config-node $N4 add-external-addr $N4_ADDR 2> /dev/null

# get peer ids
$CLI_BIN -d start-node $N1 2> /dev/null
N1_ID=$($CLI_BIN -d get-peer-id $N1 2> /dev/null)
$CLI_BIN -d start-node $N2 2> /dev/null
N2_ID=$($CLI_BIN -d get-peer-id $N2 2> /dev/null)
$CLI_BIN -d start-node $N3 2> /dev/null
N3_ID=$($CLI_BIN -d get-peer-id $N3 2> /dev/null)
$CLI_BIN -d start-node $N4 2> /dev/null
N4_ID=$($CLI_BIN -d get-peer-id $N4 2> /dev/null)
$CLI_BIN -d stop-node $N1 2> /dev/null
$CLI_BIN -d stop-node $N2 2> /dev/null
$CLI_BIN -d stop-node $N3 2> /dev/null
$CLI_BIN -d stop-node $N4 2> /dev/null

# setup bootstraps
$CLI_BIN -d config-node $N2 add-bootstrap-node "${N1_ID}" $N1_ADDR 2> /dev/null
$CLI_BIN -d config-node $N3 add-bootstrap-node "${N1_ID}" $N1_ADDR 2> /dev/null


# create files
echo "${FILE1_CONTENT}" > "$FILE1_NAME"
echo "${FILE2_CONTENT}" > "$FILE2_NAME"
echo "${FILE3_CONTENT}" > "$FILE3_NAME"
echo "${FILE4_CONTENT}" > "$FILE4_NAME"

# start nodes
$CLI_BIN -d start-node $N1 2> /dev/null
$CLI_BIN -d start-node $N2 2> /dev/null
$CLI_BIN -d start-node $N3 2> /dev/null
$CLI_BIN -d start-node $N4 2> /dev/null

# wait for nodes to connect
sleep 0.1

# publish files
$CLI_BIN -d publish-file $N1 "$FILE1_NAME" 2> /dev/null
$CLI_BIN -d publish-file $N2 "$FILE2_NAME" 2> /dev/null
$CLI_BIN -d publish-file $N3 "$FILE3_NAME" 2> /dev/null

# dial
$CLI_BIN -d dial $N4 $N3_ID $N3_ADDR 2> /dev/null

# publish the last file after dialing
$CLI_BIN -d publish-file $N4 "$FILE4_NAME" 2> /dev/null

# download files

init_asserts

RESULT11=$(cargo run -p liberum_cli download-file $N1 "${FILE1_HASH}" 2> /dev/null)
should_contain "$RESULT11" "${FILE1_CONTENT}"
RESULT12=$(cargo run -p liberum_cli download-file $N1 "${FILE2_HASH}" 2> /dev/null)
should_contain "$RESULT12" "${FILE2_CONTENT}"
RESULT13=$(cargo run -p liberum_cli download-file $N1 "${FILE3_HASH}" 2> /dev/null)
should_contain "$RESULT13" "${FILE3_CONTENT}"
RESULT14=$(cargo run -p liberum_cli download-file $N1 "${FILE4_HASH}" 2> /dev/null)
should_contain "$RESULT14" "${FILE4_CONTENT}"

RESULT21=$(cargo run -p liberum_cli download-file $N2 "${FILE1_HASH}" 2> /dev/null)
should_contain "$RESULT21" "${FILE1_CONTENT}"
RESULT22=$(cargo run -p liberum_cli download-file $N2 "${FILE2_HASH}" 2> /dev/null)
should_contain "$RESULT22" "${FILE2_CONTENT}"
RESULT23=$(cargo run -p liberum_cli download-file $N2 "${FILE3_HASH}" 2> /dev/null)
should_contain "$RESULT23" "${FILE3_CONTENT}"
RESULT24=$(cargo run -p liberum_cli download-file $N2 "${FILE4_HASH}" 2> /dev/null)
should_contain "$RESULT24" "${FILE4_CONTENT}"

RESULT31=$(cargo run -p liberum_cli download-file $N3 "${FILE1_HASH}" 2> /dev/null)
should_contain "$RESULT31" "${FILE1_CONTENT}"
RESULT32=$(cargo run -p liberum_cli download-file $N3 "${FILE2_HASH}" 2> /dev/null)
should_contain "$RESULT32" "${FILE2_CONTENT}"
RESULT33=$(cargo run -p liberum_cli download-file $N3 "${FILE3_HASH}" 2> /dev/null)
should_contain "$RESULT33" "${FILE3_CONTENT}"
RESULT34=$(cargo run -p liberum_cli download-file $N3 "${FILE4_HASH}" 2> /dev/null)
should_contain "$RESULT34" "${FILE4_CONTENT}"

RESULT41=$(cargo run -p liberum_cli download-file $N4 "${FILE1_HASH}" 2> /dev/null)
should_contain "$RESULT41" "${FILE1_CONTENT}"
RESULT42=$(cargo run -p liberum_cli download-file $N4 "${FILE2_HASH}" 2> /dev/null)
should_contain "$RESULT42" "${FILE2_CONTENT}"
RESULT43=$(cargo run -p liberum_cli download-file $N4 "${FILE3_HASH}" 2> /dev/null)
should_contain "$RESULT43" "${FILE3_CONTENT}"
RESULT44=$(cargo run -p liberum_cli download-file $N4 "${FILE4_HASH}" 2> /dev/null)
should_contain "$RESULT44" "${FILE4_CONTENT}"


exit $(check_asserts)
