#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR"/lib/asserts.sh

CORE_BIN=$1
CLI_BIN=$2

# 12D3KooWMuKGmUs6rXeNNGYKKiF53DKpeEQKr2GPNab7oUQujKj1
N1="test_n1"
N1_SEED=1
N1_ADDR="/ip6/::1/udp/52137/quic-v1"
FILE1_NAME="$PWD/test-file1.txt"
FILE1_CONTENT="11111 Hello, World! 11111"
FILE1_HASH="FhFBdCe9PqTjgptawEAxybYUTMwGDdamafRjCJ2P8Gsx"

# 12D3KooWHdZxC6sYDzNCKkHPvgdvAqESoNEb2ThCC6Pv9g6YyVTS
N2="test_n2"
N2_SEED=2
N2_ADDR="/ip6/::1/udp/52138/quic-v1"
FILE2_NAME="$PWD/test-file2.txt"
FILE2_CONTENT="22222 Hello, World! 22222"
FILE2_HASH="4Xryc3R1pQjfjLrM7yG4rkvjoDrrW9rbe9qCK24kj4Pc"

# 12D3KooWS8JMXPNwghcKvDsazagjdE2YKVW7RobBoqiVZvU38XSG
N3="test_n3"
N3_SEED=3
N3_ADDR="/ip6/::1/udp/52139/quic-v1"
FILE3_NAME="$PWD/test-file3.txt"
FILE3_CONTENT="33333 Hello, World! 33333"
FILE3_HASH="6KxBVAEgRzM9fNFo3243wefmJTqgdoTdJ4hLmkNFaxrf"

# 12D3KooWEEKLJ8c9u89npG77CPN9uJE6Jr2net22MYbzZ9n8KeTp
N4="test_n4"
N4_SEED=4
N4_ADDR="/ip6/::1/udp/52140/quic-v1"
FILE4_NAME="$PWD/test-file4.txt"
FILE4_CONTENT="44444 Hello, World! 44444"
FILE4_HASH="EdfX8prcsmXYs7FxwSjp5hqCuB3kwinWvM6KwNdFFzNj"

echo "Publish and get providers ULTRA test:"

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

# $CLI_BIN -d publish-file $N4 "$FILE4_NAME" 2> /dev/null
# if [[ $? -ne 1 ]]; then
#     echo "Should fail, 4 does not know any other nodes"
#     exit 1
# fi

# dial
$CLI_BIN -d dial $N4 $N3_ID $N3_ADDR 2> /dev/null
$CLI_BIN -d publish-file $N4 "$FILE4_NAME" 2> /dev/null
if [[ $? -ne 0 ]]; then
    echo "Should succeed, 4 knows other nodes"
    exit 1
fi


init_asserts
# download files
RESULT11=$(cargo run -p liberum_cli get-providers $N1 "${FILE1_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT11" ""
RESULT12=$(cargo run -p liberum_cli get-providers $N1 "${FILE2_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT12" ""
RESULT13=$(cargo run -p liberum_cli get-providers $N1 "${FILE3_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT13" ""
RESULT14=$(cargo run -p liberum_cli get-providers $N1 "${FILE4_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT14" ""

RESULT21=$(cargo run -p liberum_cli get-providers $N2 "${FILE1_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT21" ""
RESULT22=$(cargo run -p liberum_cli get-providers $N2 "${FILE2_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT22" ""
RESULT23=$(cargo run -p liberum_cli get-providers $N2 "${FILE3_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT23" ""
RESULT24=$(cargo run -p liberum_cli get-providers $N2 "${FILE4_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT24" ""

RESULT31=$(cargo run -p liberum_cli get-providers $N3 "${FILE1_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT31" ""
RESULT32=$(cargo run -p liberum_cli get-providers $N3 "${FILE2_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT32" ""
RESULT33=$(cargo run -p liberum_cli get-providers $N3 "${FILE3_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT33" ""
RESULT34=$(cargo run -p liberum_cli get-providers $N3 "${FILE4_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT34" ""

RESULT41=$(cargo run -p liberum_cli get-providers $N4 "${FILE1_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT41" ""
RESULT42=$(cargo run -p liberum_cli get-providers $N4 "${FILE2_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT42" ""
RESULT43=$(cargo run -p liberum_cli get-providers $N4 "${FILE3_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT43" ""
RESULT44=$(cargo run -p liberum_cli get-providers $N4 "${FILE4_HASH}" 2> /dev/null)
should_not_be_equal "$RESULT44" ""



# cleanup
$CLI_BIN -d stop-node $N1 2> /dev/null
$CLI_BIN -d stop-node $N2 2> /dev/null
$CLI_BIN -d stop-node $N3 2> /dev/null
$CLI_BIN -d stop-node $N4 2> /dev/null
killall liberum_core &> /dev/null
rm "$FILE1_NAME"
rm "$FILE2_NAME"
rm "$FILE3_NAME"
rm "$FILE4_NAME"

exit $(check_asserts)
