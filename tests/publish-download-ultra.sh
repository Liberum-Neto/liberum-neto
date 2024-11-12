#!/bin/bash

N1="test_n1"
N1_SEED=1
N1_ADDR="/ip6/::1/udp/52137/quic-v1"
FILE1_NAME="test-file1.txt"
FILE1_CONTENT="11111 Hello, World! 11111"
FILE1_HASH="FhFBdCe9PqTjgptawEAxybYUTMwGDdamafRjCJ2P8Gsx"

N2="test_n2"
N2_SEED=2
N2_ADDR="/ip6/::1/udp/52138/quic-v1"
FILE2_NAME="test-file2.txt"
FILE2_CONTENT="22222 Hello, World! 22222"
FILE2_HASH="4Xryc3R1pQjfjLrM7yG4rkvjoDrrW9rbe9qCK24kj4Pc"

N3="test_n3"
N3_SEED=3
N3_ADDR="/ip6/::1/udp/52139/quic-v1"
FILE3_NAME="test-file3.txt"
FILE3_CONTENT="33333 Hello, World! 33333"
FILE3_HASH="6KxBVAEgRzM9fNFo3243wefmJTqgdoTdJ4hLmkNFaxrf"

N4="test_n4"
N4_SEED=4
N4_ADDR="/ip6/::1/udp/52140/quic-v1"
FILE4_NAME="test-file4.txt"
FILE4_CONTENT="44444 Hello, World! 44444"
FILE4_HASH="EdfX8prcsmXYs7FxwSjp5hqCuB3kwinWvM6KwNdFFzNj"

echo "Provide and download file test:"

# run daemon
killall liberum_core &> /dev/null
nohup cargo run -p liberum_core  &> /dev/null &
sleep 0.01; # the socket file is created asynchronously and may not be ready yet :))))

# create ndoes
cargo run -p liberum_cli -- -d new-node $N1 --id-seed $N1_SEED 2> /dev/null
cargo run -p liberum_cli -- -d new-node $N2 --id-seed $N2_SEED 2> /dev/null
cargo run -p liberum_cli -- -d new-node $N3 --id-seed $N3_SEED 2> /dev/null
cargo run -p liberum_cli -- -d new-node $N4 --id-seed $N4_SEED 2> /dev/null

#config addresses
cargo run -p liberum_cli -- -d config-node $N1 add-external-addr $N1_ADDR 2> /dev/null
cargo run -p liberum_cli -- -d config-node $N2 add-external-addr $N2_ADDR 2> /dev/null
cargo run -p liberum_cli -- -d config-node $N3 add-external-addr $N3_ADDR 2> /dev/null
cargo run -p liberum_cli -- -d config-node $N4 add-external-addr $N4_ADDR 2> /dev/null

# get peer ids
cargo run -p liberum_cli -- -d start-node $N1 2> /dev/null
N1_ID=$(cargo run -p liberum_cli -- -d get-peer-id $N1 2> /dev/null)
cargo run -p liberum_cli -- -d start-node $N2 2> /dev/null
N2_ID=$(cargo run -p liberum_cli -- -d get-peer-id $N2 2> /dev/null)
cargo run -p liberum_cli -- -d start-node $N3 2> /dev/null
N3_ID=$(cargo run -p liberum_cli -- -d get-peer-id $N3 2> /dev/null)
cargo run -p liberum_cli -- -d start-node $N4 2> /dev/null
N4_ID=$(cargo run -p liberum_cli -- -d get-peer-id $N4 2> /dev/null)
cargo run -p liberum_cli -- -d stop-node $N1 2> /dev/null
cargo run -p liberum_cli -- -d stop-node $N2 2> /dev/null
cargo run -p liberum_cli -- -d stop-node $N3 2> /dev/null
cargo run -p liberum_cli -- -d stop-node $N4 2> /dev/null

# setup bootstraps
cargo run -p liberum_cli -- -d config-node $N2 add-bootstrap-node "${N1_ID}" $N1_ADDR 2> /dev/null
cargo run -p liberum_cli -- -d config-node $N3 add-bootstrap-node "${N1_ID}" $N1_ADDR 2> /dev/null


# create files
echo "${FILE1_CONTENT}" > "$FILE1_NAME"
echo "${FILE2_CONTENT}" > "$FILE2_NAME"
echo "${FILE3_CONTENT}" > "$FILE3_NAME"
echo "${FILE4_CONTENT}" > "$FILE4_NAME"

# start nodes
cargo run -p liberum_cli -- -d start-node $N1 2> /dev/null
cargo run -p liberum_cli -- -d start-node $N2 2> /dev/null
cargo run -p liberum_cli -- -d start-node $N3 2> /dev/null
cargo run -p liberum_cli -- -d start-node $N4 2> /dev/null

# publish files
cargo run -p liberum_cli -- -d publish-file $N1 "$FILE1_NAME" 2> /dev/null
cargo run -p liberum_cli -- -d publish-file $N2 "$FILE2_NAME" 2> /dev/null
cargo run -p liberum_cli -- -d publish-file $N3 "$FILE3_NAME" 2> /dev/null
cargo run -p liberum_cli -- -d publish-file $N4 "$FILE4_NAME" 2> /dev/null

# dial
cargo run -p liberum_cli -- -d dial $N4 $N3_ID $N3_ADDR 2> /dev/null

# download files
RESULT11=$(cargo run -p liberum_cli download-file $N1 "${FILE1_HASH}" 2> /dev/null)
RESULT12=$(cargo run -p liberum_cli download-file $N1 "${FILE2_HASH}" 2> /dev/null)
RESULT13=$(cargo run -p liberum_cli download-file $N1 "${FILE3_HASH}" 2> /dev/null)
RESULT14=$(cargo run -p liberum_cli download-file $N1 "${FILE4_HASH}" 2> /dev/null)

RESULT21=$(cargo run -p liberum_cli download-file $N2 "${FILE1_HASH}" 2> /dev/null)
RESULT22=$(cargo run -p liberum_cli download-file $N2 "${FILE2_HASH}" 2> /dev/null)
RESULT23=$(cargo run -p liberum_cli download-file $N2 "${FILE3_HASH}" 2> /dev/null)
RESULT24=$(cargo run -p liberum_cli download-file $N2 "${FILE4_HASH}" 2> /dev/null)

RESULT31=$(cargo run -p liberum_cli download-file $N3 "${FILE1_HASH}" 2> /dev/null)
RESULT32=$(cargo run -p liberum_cli download-file $N3 "${FILE2_HASH}" 2> /dev/null)
RESULT33=$(cargo run -p liberum_cli download-file $N3 "${FILE3_HASH}" 2> /dev/null)
RESULT34=$(cargo run -p liberum_cli download-file $N3 "${FILE4_HASH}" 2> /dev/null)

RESULT41=$(cargo run -p liberum_cli download-file $N4 "${FILE1_HASH}" 2> /dev/null)
RESULT42=$(cargo run -p liberum_cli download-file $N4 "${FILE2_HASH}" 2> /dev/null)
RESULT43=$(cargo run -p liberum_cli download-file $N4 "${FILE3_HASH}" 2> /dev/null)
RESULT44=$(cargo run -p liberum_cli download-file $N4 "${FILE4_HASH}" 2> /dev/null)

RESULT=1
if [[ ! "${RESULT11}" =~ "${FILE1_CONTENT}" ]]; then
    RESULT=0
    echo "1-1: \"${RESULT11}\" does not contain \"${FILE1_CONTENT}\""
fi
if [[ ! "${RESULT12}" =~ "${FILE2_CONTENT}" ]]; then
    RESULT=0
    echo "1-2: \"${RESULT12}\" does not contain \"${FILE2_CONTENT}\""
fi
if [[ ! "${RESULT13}" =~ "${FILE3_CONTENT}" ]]; then
    RESULT=0
    echo "1-3: \"${RESULT13}\" does not contain \"${FILE3_CONTENT}\""
fi
if [[ ! "${RESULT14}" =~ "${FILE4_CONTENT}" ]]; then
    RESULT=0
    echo "1-4: \"${RESULT14}\" does not contain \"${FILE4_CONTENT}\""
fi

if [[ ! "${RESULT21}" =~ "${FILE1_CONTENT}" ]]; then
    RESULT=0
    echo "2-1: \"${RESULT21}\" does not contain \"${FILE1_CONTENT}\""
fi
if [[ ! "${RESULT22}" =~ "${FILE2_CONTENT}" ]]; then
    RESULT=0
    echo "2-2: \"${RESULT22}\" does not contain \"${FILE2_CONTENT}\""
fi
if [[ ! "${RESULT23}" =~ "${FILE3_CONTENT}" ]]; then
    RESULT=0
    echo "2-3: \"${RESULT23}\" does not contain \"${FILE3_CONTENT}\""
fi
if [[ ! "${RESULT24}" =~ "${FILE4_CONTENT}" ]]; then
    RESULT=0
    echo "2-4: \"${RESULT24}\" does not contain \"${FILE4_CONTENT}\""
fi

if [[ ! "${RESULT31}" =~ "${FILE1_CONTENT}" ]]; then
    RESULT=0
    echo "3-1: \"${RESULT31}\" does not contain \"${FILE1_CONTENT}\""
fi
if [[ ! "${RESULT32}" =~ "${FILE2_CONTENT}" ]]; then
    RESULT=0
    echo "3-2: \"${RESULT32}\" does not contain \"${FILE2_CONTENT}\""
fi
if [[ ! "${RESULT33}" =~ "${FILE3_CONTENT}" ]]; then
    RESULT=0
    echo "3-3: \"${RESULT33}\" does not contain \"${FILE3_CONTENT}\""
fi
if [[ ! "${RESULT34}" =~ "${FILE4_CONTENT}" ]]; then
    RESULT=0
    echo "3-4: \"${RESULT34}\" does not contain \"${FILE4_CONTENT}\""
fi

if [[ ! "${RESULT41}" =~ "${FILE1_CONTENT}" ]]; then
    RESULT=0
    echo "4-1: \"${RESULT41}\" does not contain \"${FILE1_CONTENT}\""
fi
if [[ ! "${RESULT42}" =~ "${FILE2_CONTENT}" ]]; then
    RESULT=0
    echo "4-2: \"${RESULT42}\" does not contain \"${FILE2_CONTENT}\""
fi
if [[ ! "${RESULT43}" =~ "${FILE3_CONTENT}" ]]; then
    RESULT=0
    echo "4-3: \"${RESULT43}\" does not contain \"${FILE3_CONTENT}\""
fi
if [[ ! "${RESULT44}" =~ "${FILE4_CONTENT}" ]]; then
    RESULT=0
    echo "4-4: \"${RESULT44}\" does not contain \"${FILE4_CONTENT}\""
fi

# cleanup
cargo run -p liberum_cli -- -d stop-node $N1 2> /dev/null
cargo run -p liberum_cli -- -d stop-node $N2 2> /dev/null
cargo run -p liberum_cli -- -d stop-node $N3 2> /dev/null
cargo run -p liberum_cli -- -d stop-node $N4 2> /dev/null
killall liberum_core &> /dev/null
rm "$FILE1_NAME"
rm "$FILE2_NAME"
rm "$FILE3_NAME"
rm "$FILE4_NAME"

# check result
if [[ "${RESULT}" =~ "${FILE_CONTENT}" ]]; then
    echo "Success"
    exit 0
else
    echo "Failure"
    echo "\"${RESULT}\" does not contain \"${FILE_CONTENT}\""
    exit 1
fi
