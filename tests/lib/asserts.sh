set +x
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR"/colors.sh

init_asserts () {
    export ASSERT_FAILED=0
}

check_asserts() {
    return $ASSERT_FAILED
}

should_be_equal () {
    set +x
    input="$1"
    expected="$2"
    printf "${BLUE}ASSERT${NC} \"$input\" ${BLUE}should be equal to${NC} \"$expected\""
    if [ ! "$input" == "$expected" ]; then
        printf "${RED}FAILED${NC}\n"
        set -x
        ASSERT_FAILED=1
    fi
    printf "${GREEN}OK${NC}\n"
    set -x
}

should_not_be_equal () {
    set +x
    input="$1"
    expected="$2"
    printf "${BLUE}ASSERT${NC} \"$input\" ${BLUE}should not be equal to${NC} \"$expected\""
    if [ ! "$input" != "$expected" ]; then
        printf "${RED}FAILED${NC}\n"
        set -x
        ASSERT_FAILED=1
    fi
    printf "${GREEN}OK${NC}\n"
    set -x
}

should_contain () {
    set +x
    input="$1"
    expected="$2"
    printf "${BLUE}ASSERT${NC} \"$input\" ${BLUE}should contain${NC} \"$expected\""
    if [[ ! "$input" =~ "$expected" ]]; then
        printf "${RED}FAILED${NC}\n"
        set -x
        ASSERT_FAILED=1
    fi
    printf "${GREEN}OK${NC}\n"
    set -x
}

should_not_contain () {
    set +x
    input="$1"
    expected="$2"
    printf "${BLUE}ASSERT${NC} \"$input\" ${BLUE}should not contain${NC} \"$expected\""
    if [[ "$input" =~ "$expected" ]]; then
        printf "${RED}FAILED${NC}\n"
        set -x
        ASSERT_FAILED=1
    fi
    printf "${GREEN}OK${NC}\n"
    set -x
}

should_be_in () {
    set +x
    input="$1"
    expected="$2"
    printf "${BLUE}ASSERT${NC} \"$input\" ${BLUE}should be in${NC} \"$expected\""
    if [[ ! "$expected" =~ "$input" ]]; then
        printf "${RED}FAILED${NC}\n"
        set -x
        ASSERT_FAILED=1
    fi
    printf "${GREEN}OK${NC}\n"
    set -x
}

should_not_be_in () {
    set +x
    input="$1"
    expected="$2"
    printf "${BLUE}ASSERT${NC} \"$input\" ${BLUE}should not be in${NC} \"$expected\""
    if [[ "$expected" =~ "$input" ]]; then
        printf "${RED}FAILED${NC}\n"
        set -x
        ASSERT_FAILED=1
    fi
    printf "${GREEN}OK${NC}\n"
    set -x
}

set -x
