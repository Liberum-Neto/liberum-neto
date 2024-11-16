#!/bin/bash
ARGS=$*;
TESTS_DIR="tests"
TESTS=()
RESULTS=()
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

if [[ -z $ARGS ]]; then
  TO_RUN=($( ls $TESTS_DIR/*.sh ));
else
  TO_RUN=($ARGS);
fi

for test_file in ${TO_RUN[@]}; do
  if [[ ! -f $test_file ]]; then
    printf "${RED}Test file not found: ${test_file}${NC}\n"
    continue
  fi

  TESTS+=("$test_file")
  printf "${YELLOW}Running test: ${test_file}${NC}\n"
  bash -x "$test_file"
  RESULTS+=("$?")
done

printf "\n\n================================================================================\n"
printf "Results:\n"
printf "================================================================================\n\n"

PASS=1
for i in "${!RESULTS[@]}"; do
  printf "%-50s" "${TESTS[$i]}"
  if [[ ${RESULTS[$i]} -eq 0 ]]; then
    printf "${GREEN}PASS${NC}\n"
  else
    printf "${RED}FAIL${NC}\n"
    PASS=0
  fi
done

if [[ $PASS -eq 1 ]]; then
  if [[ ${#RESULTS[@]} -eq 0 ]]; then
    printf "\n\n${RED}No tests found!${NC}\n"
    exit 1
  fi
  printf "\n\n${GREEN}All tests passed!${NC}\n"
  exit 0
else
  printf "\n\n${RED}Some tests failed!${NC}\n"
  exit 1
fi
