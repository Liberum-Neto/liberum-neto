*** Settings ***
Resource    ../keywords.robot

Suite Setup    Run Keyword   Setup

Suite Teardown    Run Keyword   Teardown

*** Variables ***
${N1}=    test_n1
${N1_SEED}=    1
${N1_ADDR}=    /ip6/::1/udp/53137/quic-v1
${N1_BAD_ADDR}=    /ip6/::2/udp/53137/quic-v1
${N1_BAD_PORT}=    /ip6/::1/udp/53199/quic-v1
${N2}=    test_n2
${N2_SEED}=    2

*** Test Cases ***
Dial Nonexisting
    # Create nodes
    Run Cli     -d new-node ${N1} --id-seed ${N1_SEED} 2> /dev/null
    Run Cli     -d new-node ${N2} --id-seed ${N2_SEED} 2> /dev/null

    # Config bootstrap note
    Run Cli     -d config-node ${N1} add-external-addr ${N1_ADDR} 2> /dev/null

    # Start nodes
    Run Cli     -d start-node ${N2} 2> /dev/null
    Run Cli     -d start-node ${N1} 2> /dev/null
    ${N1_ID}=    Run Cli    -d get-peer-id ${N1} 2> /dev/null

    # Dial tests
    ${BAD1}=    Run Cli    dial ${N2} "${N1_ID}" ${N1_BAD_ADDR} 2> /dev/null
    Should Be Equal    ${BAD1}    Error dialing peer
    ${BAD2}=    Run Cli    dial ${N2} "${N1_ID}" ${N1_BAD_PORT} 2> /dev/null
    Should Be Equal    ${BAD2}    Error dialing peer
    ${GOOD}=    Run Cli    dial ${N2} "${N1_ID}" ${N1_ADDR} 2> /dev/null
    Should Be Equal    ${GOOD}    Dialing successful

    # Check if nodes did not die
    ${ALIVE}=    Run Cli    list-nodes 2> /dev/null | grep -c "true"
    Should Be Equal    ${ALIVE}    2
