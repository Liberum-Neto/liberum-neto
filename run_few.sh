
set -x
for i in {1..6}
do
    cargo run --bin=liberum_test -- --test http://192.168.1.50:1003/ $i > log$i.txt &
done
