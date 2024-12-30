*** Settings ***
Resource    ../keywords.robot

Suite Setup    Run Keywords   Setup
...    AND    Run    echo -n ${FILE_CONTENT} > ${FILE_NAME}

Suite Teardown    Run Keywords   Teardown
...    AND    Run    rm ${FILE_NAME}

Library    String

*** Variables ***
${N1}=    test_n1
${N1_SEED}=    1
${N1_ADDR}=    /ip6/::1/udp/53137/quic-v1
${FILE_NAME}=    ${CURDIR}/test_file.txt
${FILE_CONTENT}=    Hello, World!
${FILE_ID}=    3jFpcrWSnKvfEaYPCiXAujR1JiBaKVs4gEMRW4sTVn63
${N2}=    test_n2
${N2_SEED}=    2

*** Test Cases ***
Provide Download
    # Create nodes
    Run Cli     -d new-node ${N1} --id-seed ${N1_SEED} 2> /dev/null
    Run Cli     -d new-node ${N2} --id-seed ${N2_SEED} 2> /dev/null

    # Config bootstrap note
    Run Cli     -d config-node ${N1} add-external-addr ${N1_ADDR} 2> /dev/null

    # Start nodes
    Run Cli     -d start-node ${N1} 2> /dev/null
    ${N1_ID}=    Run Cli    -d get-peer-id ${N1} 2> /dev/null
    Run Cli     -d config-node ${N2} add-bootstrap-node ${N1_ID} ${N1_ADDR} 2> /dev/null
    Run Cli     -d start-node ${N2} 2> /dev/null

    Sleep    0.1s    # Wait for nodes to connect

    Run Cli     -d provide-file ${N1} ${FILE_NAME}

    ${FILE}=    Run Cli    -d download-file ${N2} ${FILE_ID} 2> /dev/null
    Should Match Regexp    ${FILE}    ${FILE_CONTENT}
