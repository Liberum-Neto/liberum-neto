#!/bin/bash

TESTS=()
RESULTS=()
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

for f in tests/*.sh; do
  TESTS+=("$f")
  bash "$f"
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
  printf "\n\n${GREEN}All tests passed!${NC}\n"
  exit 0
else
  printf "\n\n${RED}Some tests failed!${NC}\n"
  exit 1
fi
